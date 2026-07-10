#!/usr/bin/env nu
#
# Emit MCP server configuration files from a JSON source of truth.
#
# Source: config/mcp/servers.json
#   { "mcpServers": { "<name>": { ...standard fields..., "_requires": [...], "_enabled": false } } }
#   _requires : env vars that must be non-empty, or the server is skipped
#   _enabled  : false ⇒ opt-in via --enable <name>
#   Any string field may contain ${VAR} references, substituted from env.
#
# Targets: named presets that know each AI client's path, format, and merge strategy.
# Use --target codex (etc.) to write the right file in the right shape without overwriting siblings.

use std/log

const DEFAULT_CONFIG = "~/dotconfig/config/mcp/servers.json"

# Per-target: where to write, what format, replace vs merge, which top-level key holds the servers map.
# "replace" wipes the whole file. "merge" preserves all other top-level keys, replacing only `key`.
const TARGETS = {
    "claude-code":    { path: ".mcp.json",                                                       format: "json", strategy: "replace", key: "mcpServers" }
    "claude-desktop": { path: "~/Library/Application Support/Claude/claude_desktop_config.json", format: "json", strategy: "merge",   key: "mcpServers" }
    "cursor":         { path: "~/.cursor/mcp.json",                                              format: "json", strategy: "replace", key: "mcpServers" }
    "codex":          { path: "~/.codex/config.toml",                                            format: "toml", strategy: "merge",   key: "mcp_servers" }
    "zed":            { path: "~/.config/zed/settings.json",                                     format: "json", strategy: "merge",   key: "context_servers" }
    "gemini":         { path: "~/.gemini/settings.json",                                         format: "json", strategy: "merge",   key: "mcpServers" }
}

# ---------- helpers ----------

def split-csv [s: string]: nothing -> list<string> {
    $s | split row "," | each { |x| $x | str trim } | where { |x| ($x | is-not-empty) }
}

def substitute-vars [payload: any]: any -> any {
    let json_str = ($payload | to json)
    let var_names = (
        $json_str
        | parse --regex '\$\{(?<name>[A-Z_][A-Z0-9_]*)\}'
        | get name
        | uniq
    )
    mut out = $json_str
    for v in $var_names {
        let val = ($env | get --optional $v | default "")
        let escaped = ($val | str replace --all '\' '\\' | str replace --all '"' '\"')
        let placeholder = "${" + $v + "}"
        $out = ($out | str replace --all $placeholder $escaped)
    }
    $out | from json
}

def load-existing [path: string, format: string]: nothing -> any {
    if not ($path | path exists) { return null }
    let raw = (open --raw $path)
    if ($raw | str trim | is-empty) { return null }
    try {
        match $format {
            "json" => ($raw | from json),
            "toml" => ($raw | from toml),
            _ => null
        }
    } catch { |e|
        log error $"failed to parse existing ($path): ($e.msg) — refusing to overwrite"
        error make {msg: $"unparseable existing file: ($path)"}
    }
}

def serialize [data: any, format: string]: nothing -> string {
    match $format {
        "json" => ($data | to json --indent 2),
        "toml" => ($data | to toml),
        _ => (error make {msg: $"unknown format: ($format)"})
    }
}

def write-target [
    path: string,
    format: string,
    strategy: string,
    key: string,
    payload: record
] {
    let path_exp = ($path | path expand)

    let base = if $strategy == "merge" {
        let existing = (load-existing $path_exp $format)
        if $existing == null { {} } else if (($existing | describe) | str starts-with "record") { $existing } else {
            log warning $"existing ($path_exp) is not a record; treating as empty"
            {}
        }
    } else {
        {}
    }

    let final = ($base | upsert $key $payload)
    let serialized = (serialize $final $format)

    let parent = ($path_exp | path dirname)
    if not ($parent | path exists) {
        mkdir $parent
        log info $"created directory: ($parent)"
    }
    $serialized | save -f $path_exp
    log info $"wrote ($key) → ($path_exp) [($format), ($strategy)]"
}

# ---------- main ----------

export def --env "main" [
    --config: string = ""           # source JSON (default: ~/dotconfig/config/mcp/servers.json)
    --target: string = ""           # comma-separated: claude-code,cursor,codex,claude-desktop,zed,gemini
    --location: string = ""         # ad-hoc path(s), comma-separated. JSON, replace, mcpServers key.
    --enable: string = ""           # comma-separated: opt-in servers marked _enabled:false
    --disable: string = ""          # comma-separated: exclude otherwise-included servers
    --list                          # print server status table; do not write
    --list-targets                  # print target registry and detection; do not write
] {
    if $list_targets {
        let rows = ($TARGETS | items {|name def|
            let p = ($def.path | path expand)
            {
                target: $name
                path: $def.path
                format: $def.format
                strategy: $def.strategy
                key: $def.key
                exists: ($p | path exists)
            }
        })
        $rows | print
        return
    }

    let src = (if ($config | is-empty) { $DEFAULT_CONFIG } else { $config }) | path expand
    if not ($src | path exists) {
        error make {msg: $"mcp source not found: ($src)"}
    }

    let enable = (split-csv $enable)
    let disable = (split-csv $disable)
    let target_names = (split-csv $target)
    let locations = (split-csv $location)

    # Validate target names
    let known = ($TARGETS | columns)
    for n in $target_names {
        if not ($n in $known) {
            error make {msg: $"unknown target: ($n). Known: ($known | str join ', ')"}
        }
    }

    let raw = (open --raw $src | from json)
    let servers_in = ($raw | get --optional mcpServers | default {})
    if ($servers_in | columns | is-empty) {
        error make {msg: $"no servers under .mcpServers in ($src)"}
    }

    # Evaluate each server: enabled? requires-satisfied? why skipped (if so)
    let evaluated = ($servers_in | items {|name def|
        let req = ($def | get --optional _requires | default [])
        let default_enabled = ($def | get --optional _enabled | default true)
        let opt_in_requested = ($name in $enable)
        let force_disabled = ($name in $disable)
        let missing = ($req | where { |v| ($env | get --optional $v | default "") | is-empty })

        let included = (
            not $force_disabled
            and ($default_enabled or $opt_in_requested)
            and ($missing | is-empty)
        )

        let reason = if $force_disabled {
            "disabled by --disable"
        } else if not $default_enabled and not $opt_in_requested {
            "opt-in (use --enable)"
        } else if not ($missing | is-empty) {
            $"missing env: ($missing | str join ', ')"
        } else {
            "included"
        }

        {name: $name, included: $included, reason: $reason, def: $def}
    })

    if $list {
        $evaluated | select name included reason | print
        return
    }

    # Warn about names referenced in --enable/--disable that don't exist
    for n in $enable {
        if not ($n in ($servers_in | columns)) {
            log warning $"--enable ($n): no such server in ($src)"
        }
    }
    for n in $disable {
        if not ($n in ($servers_in | columns)) {
            log warning $"--disable ($n): no such server in ($src)"
        }
    }

    # Log skipped servers
    for s in ($evaluated | where { |s| not $s.included }) {
        log info $"skip ($s.name): ($s.reason)"
    }

    # Build the servers map (strip _* metadata)
    let included = ($evaluated | where included | each { |s|
        let cleaned = ($s.def | items {|k v|
            if ($k | str starts-with "_") { null } else { {key: $k, val: $v} }
        } | where { |x| $x != null })
        let entry = ($cleaned | reduce -f {} { |it acc| $acc | upsert $it.key $it.val })
        {name: $s.name, entry: $entry}
    })

    if ($included | is-empty) {
        log warning "no servers would be included"
    }

    let servers_map = ($included | reduce -f {} { |it acc| $acc | upsert $it.name $it.entry })
    let payload = (substitute-vars $servers_map)

    # Default destination if neither --target nor --location given: ./.mcp.json
    let effective_targets = if ($target_names | is-empty) and ($locations | is-empty) {
        ["claude-code"]
    } else {
        $target_names
    }

    for name in $effective_targets {
        let t = ($TARGETS | get $name)
        write-target $t.path $t.format $t.strategy $t.key $payload
    }
    for loc in $locations {
        write-target $loc "json" "replace" "mcpServers" $payload
    }
}
