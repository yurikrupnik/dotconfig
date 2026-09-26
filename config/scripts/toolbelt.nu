#!/usr/bin/env nu
# toolbelt — every tool dotconfig puts on this machine, how much each shell
# actually uses it, and whether it earns its keep versus writing/typing the
# code for the task yourself.
#
# Inventory
#   brew, cask   config/brew/Brewfile + installed-on-request formulae + casks
#   mise         `mise ls --json` (global = ~/.config/mise/config.toml)
#   cargo        config/cargo/liner.toml + $CARGO_HOME/.crates2.json
#   node         config/node/package.json + bun's global package.json
#   uv           config/uv/tools.txt + uv tool receipts
#   local        other executables in ~/.local/bin (devkit, …)
#   alias, function   config/shell/config.toml
#   script       config/scripts/*          (installed to ~/.local/bin)
#   repo-script  scripts/**/*.{nu,sh}, install.sh, bootstrap.sh
#   just         justfile recipes          (`just <recipe>`)
#
# Usage evidence, per shell
#   zsh   .zsh_history holds DISTINCT lines only (hist_ignore_all_dups), so the
#         run log written by the preexec hook in zsh/.config/zsh/.zshrc counts
#         real runs. zsh uses = max(distinct history lines, logged runs).
#   nu    history.sqlite3 (every run) + legacy history.txt
#   bash  ~/.bash_history
#   Custom code invoked by other custom code is credited transitively:
#   `u` → update → just regen → shells.nu.
#
# Value model
#   third-party  you wrote 0 lines; every use is a task done without code.
#                missing | unused | rare (<3 uses) | active (<20) | core
#                gui / editor / lib: not launched from a shell, usage n/a
#   custom       cost = lines you maintain (alias = 1, function = commands).
#                saved = keystrokes saved per use vs typing the expansion.
#                dead (0 uses) | marginal (<1 use per 10 lines) | pays off
#   gaps         command prefixes typed by hand ≥5 times with no wrapper:
#                where writing an alias/function would pay.
#
# Governance (`toolbelt govern`, read-only)
#   shells    history/rc/PATH permissions, tokens in history, committed secrets, sfw
#   gcp       gcloud identities, SA keys on disk / activated, ADC, GKE control planes
#   mcp       servers.json + every client config mcp.nu targets: drift, pinning, secrets
#   agents    Claude Code / Codex / Gemini approval + trust settings, hooks, plugins,
#             transcripts; devkit fleet agents
#   clusters  every kube context; management clusters and the children they created
#             (CAPI, Crossplane, vcluster, Flux, Argo CD), audited recursively
#
# Manage (`toolbelt manage`, AI-planned; changes only with --apply + per-action confirm)
#   An agent (claude, or omp) reviews brew/cask/mise/cargo/node/uv rows with an
#   `also`/shadowed overlap, missing/unused/rare status or drift, and picks
#   install | uninstall | declare | undeclare. toolbelt validates each pick and
#   builds the commands and declaration edits (Brewfile, liner.toml, package.json, tools.txt).

const RARE = 3
const CORE = 20
const PAYOFF = 0.1
const GAP_MIN = 5
const THIRD_PARTY = [brew cask mise cargo node uv local]
const SOURCE_ORDER = [brew cask mise cargo node uv local alias function script repo-script just]
const WRAPPERS = [sudo time nohup exec command builtin noglob env caffeinate sfw]
const RUNNERS = [nu bash sh zsh]
const JUST_VALUE_FLAGS = [-f --justfile -d --working-directory --set --shell --dotenv-path]
const GAP_IGNORE = [cd z exit clear echo man which history source export open]
const EDITOR_BIN = '(language-server|langserver|^pyright|^rust-analyzer$)'

def repo-dir []: nothing -> string {
    $env.DOTCONFIG_DIR? | default ($env.HOME | path join dotconfig)
}

# Must match the path the preexec hook in zsh/.config/zsh/.zshrc appends to.
def zsh-log-path []: nothing -> string {
    $env.XDG_STATE_HOME? | default ($env.HOME | path join .local state) | path join toolbelt zsh.tsv
}

def has-cmd [name: string]: nothing -> bool {
    (which $name | length) > 0
}

def ls-bins [dir: string]: nothing -> list<string> {
    if not ($dir | path exists) { return [] }
    ls -l $dir | where {|f| $f.type != dir and ($f.mode | str contains "x") } | get name | path basename
}

# Non-blank, non-comment lines: what you actually maintain.
def loc [file: string]: nothing -> int {
    open --raw $file | lines | where {|l| let t = $l | str trim; $t != "" and not ($t | str starts-with "#") } | length
}

def mk [r: record]: nothing -> record {
    {
        source: "" name: "" bins: [] installed: true managed: "declared"
        kind: "shell" desc: "" code: 0 saved: null body: [] pkg: null
    } | merge $r
}

# ── inventory ────────────────────────────────────────────────────────────────

def cask-bin [a: record]: nothing -> string {
    let b = $a.binary
    if ($a.target? | is-not-empty) { return ($a.target | path basename) }
    let opts = $b | get -o 1
    if ($opts | describe | str starts-with "record") and ($opts.target? | is-not-empty) {
        return ($opts.target | path basename)
    }
    $b.0 | path basename
}

def brew-items [repo: string]: nothing -> list<record> {
    if not (has-cmd brew) { return [] }
    let declared = open --raw ($repo | path join config brew Brewfile) | lines
        | parse -r '^\s*(?<type>brew|cask)\s+"(?<full>[^"]+)"[^#]*(?:#\s*(?<note>.*))?$'
        | insert name {|r| $r.full | split row "/" | last }
    let info = ^brew info --json=v2 --installed | from json
    let prefix = ^brew --prefix | str trim
    let formulae = $info.formulae | each {|f| {
        name: $f.name
        full: $f.full_name
        desc: ($f.desc? | default "")
        on_request: ($f.installed | any {|i| $i.installed_on_request? | default false })
    } }
    let casks = $info.casks | each {|c| {
        name: $c.token
        full: ($c.full_token? | default $c.token)
        desc: ($c.desc? | default "")
        bins: ($c.artifacts
            | where {|a| ($a | describe | str starts-with "record") and ($a.binary? | is-not-empty) }
            | each {|a| cask-bin $a })
    } }
    let decl_brew = $declared | where type == brew
    let decl_cask = $declared | where type == cask
    let formula_names = $decl_brew.name | append ($formulae | where on_request | get name) | uniq
    let f_items = $formula_names | par-each {|n|
        let f = $formulae | where name == $n | get -o 0
        let d = $decl_brew | where name == $n | get -o 0
        let bins = if $f == null { [$n] } else {
            (ls-bins $"($prefix)/opt/($n)/bin") ++ (ls-bins $"($prefix)/opt/($n)/sbin")
        }
        mk {
            source: brew name: $n bins: $bins installed: ($f != null)
            pkg: (if $d == null { $f.full } else { $d.full })
            managed: (if $d == null { "drift" } else { "declared" })
            kind: (if ($bins | is-empty) { "lib" } else { "shell" })
            desc: (if ($d.note? | is-not-empty) { $d.note } else { $f.desc? | default "" })
        }
    }
    let cask_names = $decl_cask.name | append $casks.name | uniq
    let c_items = $cask_names | each {|n|
        let c = $casks | where name == $n | get -o 0
        let d = $decl_cask | where name == $n | get -o 0
        let bins = $c.bins? | default []
        mk {
            source: cask name: $n bins: $bins installed: ($c != null)
            pkg: (if $d == null { $c.full } else { $d.full })
            managed: (if $d == null { "drift" } else { "declared" })
            kind: (if ($bins | is-empty) { "gui" } else { "shell" })
            desc: (if ($d.note? | is-not-empty) { $d.note } else { $c.desc? | default "" })
        }
    }
    $f_items ++ $c_items
}

def cargo-crates []: nothing -> list<record> {
    let p = $env.CARGO_HOME? | default ($env.HOME | path join .cargo) | path join .crates2.json
    if not ($p | path exists) { return [] }
    open $p | get installs | transpose key v
    | each {|r| {name: ($r.key | split row " " | first), bins: $r.v.bins} }
}

def cargo-items [repo: string, crates: list<record>]: nothing -> list<record> {
    let declared = try { open ($repo | path join config cargo liner.toml) | get packages | columns } catch { [] }
    $declared | append $crates.name | uniq | each {|n|
        let c = $crates | where name == $n | get -o 0
        mk {
            source: cargo name: $n bins: ($c.bins? | default [$n]) installed: ($c != null)
            managed: (if $n in $declared { "declared" } else { "drift" })
        }
    }
}

def mise-items [cargo_bins: list<string>]: nothing -> list<record> {
    if not (has-cmd mise) { return [] }
    ^mise ls --json | from json | transpose name versions | par-each {|t|
        let active = $t.versions | where {|v| $v.active? | default false }
        let v = if ($active | is-empty) { $t.versions | last } else { $active | first }
        let dirs = try { ^mise bin-paths $"($t.name)@($v.version)" | lines } catch { [] }
        let dirs = if ($dirs | is-empty) { [$v.install_path] } else { $dirs }
        # rust's bin dir IS ~/.cargo/bin: keep only what `cargo install` didn't put there.
        let cargo_dir = $env.CARGO_HOME? | default ($env.HOME | path join .cargo) | path join bin | path expand
        let bins = $dirs | each {|d|
            let b = ls-bins $d
            if ($d | path expand) == $cargo_dir { $b | where {|x| $x not-in $cargo_bins } } else { $b }
        } | flatten | uniq
        mk {
            source: mise name: $t.name bins: $bins
            managed: (if ($active | is-empty) { "project" } else { "global" })
        }
    }
}

def node-items [repo: string]: nothing -> list<record> {
    let declared = try { open ($repo | path join config node package.json) | get dependencies | columns } catch { [] }
    let global = $env.BUN_INSTALL? | default ($env.HOME | path join .bun) | path join install global
    let installed = try { open ($global | path join package.json) | get dependencies | columns } catch { [] }
    $declared | append $installed | uniq | each {|n|
        let pj = $global | path join node_modules $n package.json
        let bin = if ($pj | path exists) { open $pj | get -o bin } else { null }
        let bins = match ($bin | describe | str replace -r '<.*' '') {
            "string" => [($n | split row "/" | last)]
            "record" => ($bin | columns)
            _ => []
        }
        mk {
            source: node name: $n bins: $bins installed: ($n in $installed)
            managed: (if $n in $declared { "declared" } else { "drift" })
            kind: (if ($bins | is-empty) { "lib" } else if ($bins | any {|b| $b =~ $EDITOR_BIN }) { "editor" } else { "shell" })
        }
    }
}

def uv-tools-dir []: nothing -> string {
    $env.UV_TOOL_DIR? | default ($env.HOME | path join .local share uv tools)
}

def uv-items [repo: string]: nothing -> list<record> {
    let f = $repo | path join config uv tools.txt
    let declared = if ($f | path exists) {
        open --raw $f | lines
        | each {|l| $l | str replace -r '#.*' '' | str trim }
        | where $it != ""
        | each {|l| let spec = $l | split row -r '\s+' | first; {name: ($spec | str replace -r '[\[=<>~!;].*' ''), spec: $spec} }
    } else { [] }
    let dir = uv-tools-dir
    let installed = if ($dir | path exists) {
        ls $dir | where type == dir | each {|d|
            let r = $d.name | path join uv-receipt.toml
            let bins = if ($r | path exists) { open $r | get -o tool.entrypoints | default [] | get name } else { [] }
            {name: ($d.name | path basename), bins: $bins}
        }
    } else { [] }
    $declared.name | append $installed.name | uniq | each {|n|
        let i = $installed | where name == $n | get -o 0
        mk {
            source: uv name: $n bins: ($i.bins? | default [$n]) installed: ($i != null)
            pkg: ($declared | where name == $n | get -o 0.spec | default $n)
            managed: (if $n in $declared.name { "declared" } else { "drift" })
        }
    }
}

# ~/.local/bin entries not produced by this repo's generator or by uv.
def local-items [repo: string]: nothing -> list<record> {
    let dir = $env.HOME | path join .local bin
    if not ($dir | path exists) { return [] }
    let output = $repo | path join output
    let uv = uv-tools-dir
    ls $dir | where type != dir | get name
    | where {|p| let real = $p | path expand; not ($real | str starts-with $output) and not ($real | str starts-with $uv) }
    | each {|p| let n = $p | path basename; mk {source: local name: $n bins: [$n] managed: "external"} }
}

def script-desc [file: string]: nothing -> string {
    open --raw $file | lines | skip 1 | where {|l| $l =~ '^#\s*\S' } | get -o 0 | default ""
    | str replace -r '^#\s*' ''
}

def custom-items [repo: string]: nothing -> list<record> {
    let cfg = open ($repo | path join config shell config.toml)
    let aliases = $cfg.aliases? | default {} | transpose name exp | each {|a|
        mk {
            source: alias name: $a.name bins: [$a.name] managed: "dotconfig" desc: $a.exp
            code: 1 saved: (($a.exp | str length) - ($a.name | str length))
        }
    }
    let functions = $cfg.functions? | default {} | transpose name f | each {|f|
        let cmds = $f.f.commands
        mk {
            source: function name: $f.name bins: [$f.name] managed: "dotconfig"
            desc: ($f.f.description? | default "") code: ($cmds | length) body: $cmds
            saved: (($cmds | str join " && " | str length) - ($f.name | str length))
        }
    }
    let scripts = ls ($repo | path join config scripts)
    | where {|f| $f.type == file and not (($f.name | path basename) starts-with ".") and ($f.name | path basename) != README.md }
    | each {|f|
        let n = $f.name | path basename | str replace -r '\.[^.]+$' ''
        mk {
            source: script name: $n bins: [$n] managed: "dotconfig"
            desc: (script-desc $f.name) code: (loc $f.name)
        }
    }
    let repo_scripts = glob $"($repo)/scripts/**/*.{nu,sh}"
    | append ([install.sh bootstrap.sh] | each {|f| $repo | path join $f } | where {|p| $p | path exists })
    | each {|f|
        let n = $f | path basename
        mk {
            source: repo-script name: $n bins: [$n] managed: "dotconfig"
            desc: ($f | path relative-to $repo) code: (loc $f)
            body: (if ($n | str ends-with ".sh") { open --raw $f | lines } else { [] })
        }
    }
    $aliases ++ $functions ++ $scripts ++ $repo_scripts ++ (just-items $repo)
}

def just-items [repo: string]: nothing -> list<record> {
    let jf = $repo | path join justfile
    if not ((has-cmd just) and ($jf | path exists)) { return [] }
    let dump = ^just -f $jf --dump --dump-format json | from json
    let vars = $dump.assignments | transpose name a | each {|v| {name: $v.name, value: ($v.a.value | into string)} }
    $dump.recipes | transpose name r | each {|r|
        let lines = $r.r.body | each {|line|
            $line | each {|frag|
                if ($frag | describe | str starts-with "string") { $frag } else {
                    let var = $frag | flatten | get -o 1
                    $vars | where name == $var | get -o 0.value | default ""
                }
            } | str join "" | str trim -l -c "@" | str trim -l -c "-"
        }
        let deps = $r.r.dependencies | each {|d| $"just ($d.recipe)" }
        mk {
            source: just name: $r.name bins: [$"just ($r.name)"] managed: "dotconfig"
            desc: ($r.r.doc? | default "") body: ($lines ++ $deps)
            code: ([($lines | where {|l| ($l | str trim) != "" } | length) 1] | math max)
        }
    }
}

def inventory [repo: string]: nothing -> list<record> {
    let crates = cargo-crates
    let cargo_bins = $crates | get bins | flatten
    let parts = [brew mise cargo node uv local custom] | par-each {|s|
        match $s {
            "brew" => (brew-items $repo)
            "mise" => (mise-items $cargo_bins)
            "cargo" => (cargo-items $repo $crates)
            "node" => (node-items $repo)
            "uv" => (uv-items $repo)
            "local" => (local-items $repo)
            "custom" => (custom-items $repo)
        }
    }
    $parts | flatten | insert id {|it| $"($it.source):($it.name)" }
}

# ── usage evidence ───────────────────────────────────────────────────────────

def epoch [n: int, unit: string]: nothing -> datetime {
    match $unit { "s" => ($n * 1_000_000_000 | into datetime), _ => ($n * 1_000_000 | into datetime) }
}

def read-zsh-history [path: string]: nothing -> list<record> {
    # zsh stores embedded newlines as backslash-newline: rejoin before splitting.
    open --raw $path | decode utf-8 | str replace -a "\\\n" " " | lines | where $it != "" | each {|l|
        if ($l | str starts-with ": ") {
            let m = $l | parse -r '^: (?<ts>\d+):\d+;(?<cmd>.*)$'
            if ($m | is-empty) { {src: zh, ts: null, line: $l} } else {
                {src: zh, ts: (epoch ($m.0.ts | into int) s), line: $m.0.cmd}
            }
        } else { {src: zh, ts: null, line: $l} }
    }
}

def zsh-history-paths []: nothing -> list<string> {
    [($env.ZDOTDIR? | default ($env.HOME | path join .config zsh) | path join .zsh_history)
     ($env.HOME | path join .zsh_history)]
    | uniq | where {|p| $p | path exists }
}

def nu-db-path []: nothing -> string { $env.HOME | path join .config nushell history.sqlite3 }
def nu-txt-path []: nothing -> string { $env.HOME | path join .config nushell history.txt }
def bash-history-path []: nothing -> string { $env.HOME | path join .bash_history }

def invocations []: nothing -> list<record> {
    let zh = zsh-history-paths | each {|p| read-zsh-history $p } | flatten
    let zl = if (zsh-log-path | path exists) {
        open --raw (zsh-log-path) | decode utf-8 | lines
        | parse -r '^(?<ts>\d+)\t(?<line>.*)$'
        | each {|r| {src: zl, ts: (epoch ($r.ts | into int) s), line: $r.line} }
    } else { [] }
    let nd = if (nu-db-path | path exists) {
        open (nu-db-path) | query db "SELECT command_line AS line, start_timestamp AS ts FROM history"
        | each {|r| {src: nd, ts: (if $r.ts == null { null } else { epoch $r.ts ms }), line: $r.line} }
    } else { [] }
    let nt = if (nu-txt-path | path exists) {
        open --raw (nu-txt-path) | lines | where $it != "" | each {|l| {src: nt, ts: null, line: $l} }
    } else { [] }
    let bh = if (bash-history-path | path exists) {
        open --raw (bash-history-path) | lines | where {|l| $l != "" and not ($l =~ '^#\d+$') }
        | each {|l| {src: bh, ts: null, line: $l} }
    } else { [] }
    $zh ++ $zl ++ $nd ++ $nt ++ $bh
}

# One pipeline segment → usage tokens + the first plain words as typed.
# tokens: head (basename if a path), `just <recipe>`, script run via
#         `nu x.nu`/`bash x.sh`, command behind `sfw`, alias expansion head.
def seg-parse [seg: string, aliases: record]: nothing -> record {
    mut words = $seg | str trim | str trim -l -c "(" | str trim -l -c "{" | split row -r '\s+' | where $it != ""
    mut tokens = []
    while ($words | is-not-empty) and (($words.0 =~ '^[A-Za-z_][A-Za-z0-9_]*=') or ($words.0 in $WRAPPERS)) {
        if $words.0 == "sfw" { $tokens = $tokens | append "sfw" }
        $words = $words | skip 1
    }
    if ($words | is-empty) { return {tokens: $tokens, words: []} }
    let head = $words.0 | str trim -l -c "^"
    let args = $words | skip 1
    $tokens = $tokens | append (if ($head | str contains "/") { $head | path basename } else { $head })
    if $head == "just" {
        mut skip_next = false
        for a in $args {
            if $skip_next { $skip_next = false; continue }
            if ($a | str starts-with "-") { $skip_next = ($a in $JUST_VALUE_FLAGS); continue }
            $tokens = $tokens | append $"just ($a)"
            break
        }
    } else if $head in $RUNNERS {
        let script = $args | where {|a| not ($a | str starts-with "-") } | get -o 0
        if ($script != null) and ($script =~ '\.(nu|sh)$') { $tokens = $tokens | append ($script | path basename) }
    } else if $head == "cargo" {
        # `cargo nextest` runs the cargo-nextest binary
        let sub = $args | where {|a| not ($a | str starts-with "-") and not ($a | str starts-with "+") } | get -o 0
        if $sub != null { $tokens = $tokens | append $"cargo-($sub)" }
    }
    if ($head in $aliases) {
        let expanded = [($aliases | get $head)] ++ $args | str join " "
        $tokens = $tokens | append (seg-parse $expanded {} | get tokens)
    }
    let plain = $words | take while {|w| $w =~ '^[A-Za-z][A-Za-z0-9._:@-]*$' } | first 3
    {tokens: $tokens, words: $plain}
}

def line-parse [line: string, aliases: record]: nothing -> record {
    let segs = $line | split row -r '\|\||&&|;|\||\$\(|`' | each {|s| seg-parse $s $aliases }
    {
        tokens: ($segs | get tokens | flatten | uniq)
        prefixes: ($segs | get words | where {|w| ($w | length) >= 2 and ($w.0 not-in $GAP_IGNORE) }
            | each {|w| [($w | first 2 | str join " ")] ++ (if ($w | length) == 3 { [($w | str join " ")] } else { [] }) }
            | flatten | uniq)
    }
}

# rows {src, ts, key} → record key → {zh zl nd nt bh last}
def aggregate [rows: list<record>]: nothing -> record {
    if ($rows | is-empty) { return {} }
    $rows | group-by key | transpose key rows | each {|g|
        let r = $g.rows
        let ts = $r | get ts | compact
        {key: $g.key, stats: {
            zh: ($r | where src == zh | length)
            zl: ($r | where src == zl | length)
            nd: ($r | where src == nd | length)
            nt: ($r | where src == nt | length)
            bh: ($r | where src == bh | length)
            last: (if ($ts | is-empty) { null } else { $ts | sort | last })
        }}
    } | transpose -r -d
}

def shell-uses [s: record]: nothing -> record {
    {zsh: ([$s.zh $s.zl] | math max), nu: ($s.nd + $s.nt), bash: $s.bh}
}

def sum-stats [stats: list<record>]: nothing -> record {
    if ($stats | is-empty) { return {zh: 0, zl: 0, nd: 0, nt: 0, bh: 0, last: null} }
    let ts = $stats | get last | compact
    {
        zh: ($stats | get zh | append 0 | math sum)
        zl: ($stats | get zl | append 0 | math sum)
        nd: ($stats | get nd | append 0 | math sum)
        nt: ($stats | get nt | append 0 | math sum)
        bh: ($stats | get bh | append 0 | math sum)
        last: (if ($ts | is-empty) { null } else { $ts | sort | last })
    }
}

# ── scoring ──────────────────────────────────────────────────────────────────

def status-of [it: record]: nothing -> string {
    if $it.source in $THIRD_PARTY {
        if not $it.installed { return "missing" }
        if $it.shadowed { return "shadowed" }
        if $it.uses == 0 and $it.kind != "shell" { return $it.kind }
        if $it.uses == 0 { return "unused" }
        if $it.uses < $RARE { return "rare" }
        if $it.uses < $CORE { return "active" }
        return "core"
    }
    if $it.uses == 0 { return "dead" }
    if ($it.uses / ([$it.code 1] | math max)) >= $PAYOFF { "pays off" } else { "marginal" }
}

def paint [status: string]: nothing -> string {
    let c = match $status {
        "core" | "pays off" => "green_bold"
        "active" => "green"
        "rare" | "marginal" => "yellow"
        "unused" | "dead" => "red"
        "missing" => "magenta"
        "shadowed" => "light_purple"
        _ => "dark_gray"
    }
    $"(ansi $c)($status)(ansi reset)"
}

def bar [n: number, max: number, width: int = 20]: nothing -> string {
    let w = if $max <= 0 { 0 } else { ($n / $max * $width) | math ceil | into int }
    $"(ansi cyan)('' | fill -c '█' -w $w)(ansi reset)"
}

# Callers of custom code: functions, just recipes and repo shell scripts whose
# body invokes the item. Uses flow down the call graph.
def propagate [items: list<record>, aliases: record]: nothing -> list<record> {
    let callers = $items | where {|i| $i.body | is-not-empty } | each {|i| {
        id: $i.id
        toks: ($i.body | each {|l| line-parse $l $aliases | get tokens } | flatten | uniq)
    } }
    let targets = [function script repo-script just]
    let with_callers = $items | each {|it|
        let by = if $it.source in $targets {
            $callers | where {|c| $c.id != $it.id and ($it.bins | any {|b| $b in $c.toks }) } | get id
        } else { [] }
        $it | insert callers $by
    }
    mut total = $with_callers | select id direct | transpose -r -d
    for _ in 1..6 {
        let prev = $total
        $total = $with_callers | each {|it| {
            id: $it.id
            v: ($it.direct + ($it.callers | each {|c| $prev | get $c } | append 0 | math sum))
        } } | transpose -r -d
    }
    let final = $total
    $with_callers | each {|it| $it | insert via (($final | get $it.id) - $it.direct) }
}

def gaps [prefix_rows: list<record>, items: list<record>]: nothing -> list<record> {
    let stats = aggregate $prefix_rows | transpose key s | each {|r|
        {key: $r.key, n: (shell-uses $r.s | values | math sum), words: ($r.key | split row " " | length)}
    }
    let three = $stats | where words == 3 | insert parent {|r| $r.key | split row " " | first 2 | str join " " }
    let aliases = $items | where source == alias
    let ranked = $stats | where {|r| $r.words == 2 and $r.n >= $GAP_MIN }
    | each {|p|
        let ext = $three | where parent == $p.key | sort-by n --reverse | get -o 0
        if ($ext != null) and ($ext.n >= ($p.n * 0.8)) { $ext } else { $p }
    }
    | where {|g| ($g.key | str length) >= 10 }
    | uniq-by key
    | insert saved {|g| $g.n * (($g.key | str length) - $g.words) }
    | sort-by saved --reverse
    | first 50
    # Suggest initials; on collision with an alias, command or earlier suggestion,
    # grow with letters from the last word (`gc` → `gch`).
    $ranked | reduce -f {rows: [], taken: $aliases.name} {|g, acc|
        let words = $g.key | split row " "
        let covered = $aliases | where desc == $g.key | get -o 0.name
        let base = $words | each {|w| $w | str substring 0..0 } | str join ""
        let tail = $words | last | str substring 1..
        let grow = if ($tail | is-empty) { [] } else { 1..($tail | str length) | each {|i| $base + ($tail | str substring 0..<$i) } }
        let options = [$base] ++ $grow
        let pick = $options | where {|o| $o not-in $acc.taken and not (has-cmd $o) } | get -o 0
        let fix = if $covered != null { $"use existing alias `($covered)`" } else if $pick == null { "function" } else { $"alias `($pick)`" }
        {
            rows: ($acc.rows | append {typed: $g.key, times: $g.n, chars: ($g.key | str length), saved_if_aliased: $g.saved, fix: $fix})
            taken: ($acc.taken | append ($pick | default []))
        }
    } | get rows
}

# Tools with no interactive runs may still be load-bearing: started by shell
# init (zoxide, starship, direnv) or by the editor (LSPs, formatters, rg).
def config-refs [repo: string]: nothing -> record {
    let rc = [zsh/.config/zsh/.zshrc zsh/.zshenv nushell/.config/nushell/config.nu nushell/.config/nushell/env.nu]
        | each {|f| $repo | path join $f } | where {|p| $p | path exists }
        | each {|p| open --raw $p | lines | where {|l| not ($l | str trim | str starts-with "#") } | str join "\n" }
        | str join "\n"
        | parse -r '(?m)(?:^\s*|\$\(|which\s+|\^)(?<w>[A-Za-z][\w.-]*)' | get w | uniq
    let editor = glob $"($repo)/{nvim,zed}/.config/**/*.{lua,json}"
        | where {|p| ($p | path basename) != "lazy-lock.json" }
        | each {|p| open --raw $p | lines
            | where {|l| not ($l =~ '^\s*(--|//)') and not ($l =~ 'filetypes|root_markers|ensure_installed|\bft\s*=') } }
        | flatten | str join "\n"
        | parse -r `['"](?<w>[A-Za-z][\w.-]*)[\s'"]` | get w | uniq
    {rc: $rc, editor: $editor}
}

# ── assembly ─────────────────────────────────────────────────────────────────

def load []: nothing -> record {
    let repo = repo-dir
    let jobs = [inv hist] | par-each --keep-order {|j|
        if $j == "inv" { inventory $repo } else { invocations }
    }
    let refs = config-refs $repo
    let items = $jobs.0 | each {|it|
        if not ($it.source in $THIRD_PARTY and $it.kind == "shell") { return $it }
        if ($it.bins | any {|b| $b in $refs.rc }) { return ($it | update kind init) }
        if ($it.bins | any {|b| $b in $refs.editor }) { return ($it | update kind editor) }
        $it
    }
    let inv = $jobs.1
    let aliases = $items | where source == alias | select name desc | transpose -r -d
    let parsed = $inv | par-each {|i| $i | merge (line-parse $i.line $aliases) }
    let counts = aggregate ($parsed | each {|p| $p.tokens | each {|t| {src: $p.src, ts: $p.ts, key: $t} } } | flatten)
    let prefix_rows = $parsed | each {|p| $p.prefixes | each {|k| {src: $p.src, ts: $p.ts, key: $k} } } | flatten

    # A bin shipped by several managers is credited to the one that wins on PATH
    # (zshrc order: mise-global, bun, ~/.local/bin, brew, ~/.cargo/bin; project-
    # only mise tools are off PATH outside their project). Missing items own nothing.
    let owners = $items | where {|it| $it.source in $THIRD_PARTY and $it.installed }
        | each {|it|
            let rank = match [$it.source $it.managed] {
                ["mise" "global"] => 0, ["node" _] => 1, ["uv" _] | ["local" _] => 2
                ["brew" _] | ["cask" _] => 3, ["cargo" _] => 4, _ => 5
            }
            $it.bins | each {|b| {bin: $b, source: $it.source, id: $it.id, rank: $rank} }
        } | flatten
        | group-by bin
    let scored = $items | each {|it|
        let claims = $it.bins | uniq | each {|b| {bin: $b, owners: ($owners | get -o $b | default [] | sort-by rank)} }
        let mine = if not $it.installed { [] } else if $it.source in $THIRD_PARTY {
            $claims | where {|c| ($c.owners | get -o 0.id) == $it.id } | get bin
        } else { $it.bins }
        let others = if $it.source in $THIRD_PARTY { $claims | each {|c| $c.owners | where {|o| $o.id != $it.id } } | flatten } else { [] }
        let shadowed_by = $others | where {|o| $o.bin not-in $mine } | each {|o| $o.source } | uniq
        let also = $others | where {|o| $o.bin in $mine } | each {|o| $o.source } | uniq
        let lost = if ($shadowed_by | is-empty) { [] } else if ($mine | is-empty) { [$"shadowed by ($shadowed_by | str join ',')"] } else { [$"partly shadowed by ($shadowed_by | str join ',')"] }
        let s = sum-stats ($mine | each {|b| $counts | get -o $b } | compact)
        let u = shell-uses $s
        let overlap = (if ($also | is-empty) { [] } else { [$"also ($also | str join ',')"] }) ++ $lost
        $it | merge $u | insert direct ($u.zsh + $u.nu + $u.bash) | insert last $s.last
        | insert overlap ($overlap | str join "; ")
        | insert shadowed ($it.installed and ($mine | is-empty) and ($shadowed_by | is-not-empty))
    }
    let tools = propagate $scored $aliases | each {|it|
        let row = $it | insert uses ($it.direct + $it.via)
        $row | insert status (status-of $row)
    }
    let zl_runs = $inv | where src == zl
    {
        repo: $repo
        tools: ($tools
            | insert ord {|t| $SOURCE_ORDER | enumerate | where item == $t.source | get -o 0.index | default 99 }
            | insert neg {|t| 0 - $t.uses }
            | sort-by ord neg | reject ord neg)
        inv: $inv
        gaps: (gaps $prefix_rows $tools)
        evidence: {
            zh: ($inv | where src == zh | length)
            zl: ($zl_runs | length)
            zl_since: ($zl_runs | get ts | compact | sort | get -o 0)
            nd: ($inv | where src == nd | length)
            nt: ($inv | where src == nt | length)
            bh: ($inv | where src == bh | length)
        }
    }
}

def fmt-date [d: any]: nothing -> string {
    if $d == null { "" } else { $d | format date "%Y-%m-%d" }
}

def tools-view [tools: list<record>]: nothing -> list<record> {
    $tools | each {|t| {
        source: $t.source name: $t.name status: (paint $t.status) uses: $t.uses
        zsh: $t.zsh nu: $t.nu bash: $t.bash last: (fmt-date $t.last)
        managed: $t.managed also: $t.overlap desc: ($t.desc | str substring 0..59)
    } }
}

def value-rows [tools: list<record>]: nothing -> list<record> {
    $tools | where source not-in $THIRD_PARTY | each {|t| {
        source: $t.source name: $t.name status: $t.status uses: $t.uses
        direct: $t.direct via: $t.via code: $t.code
        saved_per_use: $t.saved
        saved_total: (if $t.saved == null { null } else { $t.saved * $t.uses })
        uses_per_10_loc: ($t.uses * 10 / ([$t.code 1] | math max) | math round)
    } } | sort-by uses_per_10_loc --reverse
}

def shells-rows [data: record]: nothing -> list<record> {
    let e = $data.evidence
    let login = $env.SHELL? | default "" | path basename
    let log_note = if $e.zl == 0 { "run log empty (starts in new zsh sessions)" } else { $"($e.zl) runs logged since (fmt-date $e.zl_since)" }
    [
        {shell: zsh, wired: ($env.HOME | path join .config zsh generated.zsh), evidence: $"($e.zh) distinct history lines · ($log_note)"}
        {shell: nu, wired: ($env.HOME | path join .config nushell generated.nu), evidence: $"($e.nd) runs \(sqlite\) · ($e.nt) legacy lines"}
        {shell: bash, wired: "", evidence: $"($e.bh) history lines"}
    ]
    | where {|s| has-cmd $s.shell }
    | each {|s|
        let col = $s.shell
        let used = $data.tools | where {|t| ($t | get $col) > 0 }
        let version = if $col == "nu" { ^nu --version | str trim } else {
            ^$col --version | lines | first | parse -r '(?<v>\d+\.\d+(\.\d+)?)' | get -o 0.v | default "?"
        }
        {
            shell: $col
            version: $version
            login: ($login == $col)
            wired: (($s.wired != "") and ($s.wired | path exists))
            evidence: $s.evidence
            tools_used: ($used | length)
            invocations: ($used | where source in $THIRD_PARTY | each {|t| $t | get $col } | append 0 | math sum)
            top: ($used | where source not-in [alias] | sort-by {|t| $t | get $col } --reverse | first 6
                | each {|t| {name: $t.name, uses: ($t | get $col)} })
        }
    }
}

def shell-cards [rows: list<record>]: nothing -> nothing {
    let max = $rows.invocations | append 0 | math max
    for r in $rows {
        let login = if $r.login { $"(ansi yellow)● login(ansi reset)" } else { $"(ansi dark_gray)○ login(ansi reset)" }
        let wired = if $r.wired { $"(ansi green)✓ dotconfig-wired(ansi reset)" } else { $"(ansi dark_gray)– not managed by dotconfig(ansi reset)" }
        let top = $r.top | each {|t| $"($t.name) (ansi dark_gray)($t.uses)(ansi reset)" } | str join " · "
        print $" (ansi green_bold)($r.shell | fill -w 5)(ansi reset) ($r.version | fill -w 8) ($login)   ($wired)"
        print $"   (ansi dark_gray)evidence(ansi reset)  ($r.evidence)"
        print $"   (ansi dark_gray)usage   (ansi reset)  ($r.tools_used) tools · ($r.invocations) tool runs  (bar $r.invocations $max 30)"
        if $top != "" { print $"   (ansi dark_gray)top     (ansi reset)  ($top)" }
    }
}

def sources-rows [tools: list<record>]: nothing -> list<record> {
    let groups = $SOURCE_ORDER | each {|s| {source: $s, t: ($tools | where source == $s)} } | where {|g| $g.t | is-not-empty }
    let max = $groups | each {|g| $g.t.uses | append 0 | math sum } | append 0 | math max
    $groups | each {|g|
        let uses = $g.t.uses | append 0 | math sum
        let bad = $g.t | where status in [unused dead missing shadowed] | length
        {
            source: $g.source
            items: ($g.t | length)
            used: ($g.t | where uses > 0 | length)
            idle: (if $bad > 0 { $"(ansi red)($bad)(ansi reset)" } else { "0" })
            "n/a": ($g.t | where status in [gui editor lib init] | length)
            uses: $uses
            share: (bar $uses $max)
        }
    }
}

def header [title: string]: nothing -> nothing {
    print ""
    print $"(ansi cyan_bold)── ($title) (ansi reset)"
}

# Dashboard: shells, sources, value vs code, removal candidates, gaps.
def main [] {
    let d = load
    let t = $d.tools
    print $"(ansi white_bold)toolbelt(ansi reset) (ansi dark_gray)· ($d.repo) · (date now | format date '%Y-%m-%d %H:%M')(ansi reset)"

    header "SHELLS"
    shell-cards (shells-rows $d)

    header "SOURCES"
    print (sources-rows $t | table -i false)

    let third = $t | where {|r| $r.source in $THIRD_PARTY and $r.installed }
    let shell3 = $third | where kind == shell
    let custom = $t | where source not-in $THIRD_PARTY
    let saved = $custom | where saved != null | each {|c| $c.saved * $c.uses } | append 0 | math sum
    let loc = $custom | get code | append 0 | math sum
    header "VALUE — off-the-shelf vs code you wrote"
    print ([
        {
            layer: "off-the-shelf"
            items: ($third | length)
            used: ($shell3 | where uses > 0 | length)
            uses: ($third.uses | append 0 | math sum)
            "your LOC": 0
            "keys saved": ""
            verdict: $"($shell3 | where status == core | length) core · ($shell3 | where status == unused | length) unused"
        }
        {
            layer: "custom"
            items: ($custom | length)
            used: ($custom | where uses > 0 | length)
            uses: ($custom.uses | append 0 | math sum)
            "your LOC": $loc
            "keys saved": $saved
            verdict: $"($custom | where status == 'pays off' | length) pay off · ($custom | where status == marginal | length) marginal · ($custom | where status == dead | length) dead"
        }
    ] | table -i false)

    header "CUSTOM CODE — uses per 10 lines maintained"
    print (value-rows $t | first 12 | update status {|r| paint $r.status } | table -i false)

    header "UNUSED — installed but never run, or shadowed on PATH by another manager"
    let unused = $t | where status in [unused dead shadowed]
    print ($SOURCE_ORDER | each {|s|
        let n = $unused | where source == $s
        if ($n | is-empty) { null } else {
            let names = $n | each {|u| if $u.status == "shadowed" { $"(ansi light_purple)($u.name)(ansi reset)" } else { $u.name } }
            {source: $s, count: ($n | length), names: ($names | str join " ")}
        }
    } | compact | table -i false)
    print $"(ansi dark_gray)white = never run · (ansi light_purple)purple(ansi dark_gray) = shadowed: same bin comes from another manager first(ansi reset)"

    header $"GAPS — typed by hand ≥($GAP_MIN)×, no wrapper"
    print ($d.gaps | first 10 | table -i false)
    print $"(ansi dark_gray)drill down: toolbelt tools | shells | value | gaps | ui | govern | manage   \(--json for data\)(ansi reset)"
}

# Every inventoried tool with status and per-shell usage.
def "main tools" [
    --source (-s): string   # brew|cask|mise|cargo|node|uv|local|alias|function|script|repo-script|just
    --status: string        # core|active|rare|unused|missing|gui|editor|lib|pays off|marginal|dead
    --shell: string         # only tools used in this shell: zsh|nu|bash
    --json                  # machine-readable output
] {
    mut t = load | get tools
    if $source != null { $t = $t | where source == $source }
    if $status != null { $t = $t | where status == $status }
    if $shell != null { let col = $shell; $t = $t | where {|r| ($r | get $col) > 0 } }
    if $json { $t | reject body id callers | to json } else { tools-view $t }
}

# Per-shell panel: version, dotconfig wiring, evidence, top tools.
def "main shells" [--json] {
    let rows = shells-rows (load)
    if $json { $rows | to json } else { shell-cards $rows }
}

# Custom code ROI: uses (direct + via callers), lines maintained, keystrokes saved.
def "main value" [--json] {
    let rows = value-rows (load | get tools)
    if $json { $rows | to json } else { $rows | update status {|r| paint $r.status } }
}

# Commands typed by hand repeatedly with no alias/function: where code would pay.
def "main gaps" [--limit (-n): int = 25, --json] {
    let rows = load | get gaps | first $limit
    if $json { $rows | to json } else { $rows }
}

# Interactive browser over all tools (nu `explore`; `:q` or Esc to leave).
def "main ui" [] {
    tools-view (load | get tools) | explore
}

# ── manage ───────────────────────────────────────────────────────────────────
# `toolbelt manage`: an AI agent reviews every third-party row with something to
# fix: bins another manager also ships (`also`, shadowed), status (missing,
# unused, rare) and drift (installed but undeclared). It picks actions from a
# closed set; toolbelt checks each pick against the row's allowed actions and
# builds the commands itself, so the agent never writes shell. Nothing changes
# without --apply, and --apply confirms every action.

const MANAGEABLE = [brew cask mise cargo node uv]
const DECLARABLE = [brew cask cargo node uv]
const PLAN_SCHEMA = {
    type: object
    additionalProperties: false
    required: [summary actions]
    properties: {
        summary: {type: string}
        actions: {
            type: array
            items: {
                type: object
                additionalProperties: false
                required: [source name action reason]
                properties: {
                    source: {type: string, enum: [brew cask mise cargo node uv]}
                    name: {type: string}
                    action: {type: string, enum: [install uninstall declare undeclare]}
                    reason: {type: string}
                }
            }
        }
    }
}
const MANAGE_BRIEF = r#'You manage the software installed on one developer's macOS machine. A dotfiles repo declares what should be installed; `toolbelt` measured every package-manager row below from the machine and the user's shell history. Decide what to change.

PATH order, first wins: mise global, node (bun global), uv, brew/cask, cargo. A mise "project" install is only on PATH inside the repo whose mise.toml pins it.

Row fields:
- status: missing (declared, not installed) | shadowed (installed, but every bin is served by another manager first) | unused (installed, 0 runs) | rare (<3 runs) | active (<20) | core | gui, editor, lib, init (not run from a shell: launched by the GUI, the editor, or shell init; 0 uses is NOT evidence of disuse)
- uses, zsh, nu, bash: runs counted from shell history, including runs through the user's own aliases/functions/scripts. last: date of the last run.
- managed: declared (listed in the repo's Brewfile / liner.toml / package.json / uv tools.txt) | drift (installed but undeclared, so a new machine will not get it) | global, project (mise)
- also: "also X" = manager X ships the same bins but loses on PATH; "shadowed by X" = X wins, so this copy never runs.
- allowed: the only actions valid for this row. Never pick anything else.

Actions:
- install: install a declared-but-missing row.
- uninstall: remove it from the machine; a declared row is also removed from its declaration file.
- declare: add a drift row to its declaration file so every machine gets it.
- undeclare: drop a declaration without touching the machine (for example, a missing entry that is a typo or duplicates an installed row, such as a case mismatch).

Goals, in priority order:
1. One manager per binary. For every also/shadowed pair, keep the copy that wins PATH, or the declared one, and uninstall the other. A mise project copy pins a version for some repo; removing it frees disk, and that repo reinstalls it with `mise install`.
2. Declarations match the machine. Declare drift that is used, or that plausibly serves the editor, shell init, or another tool. Uninstall drift that is unused and serves nothing. For a missing row: install it if it looks wanted; undeclare it if it looks wrong.
3. Unused declared tools: uninstall only when clearly unneeded, for example 0 runs, no editor/init role, and superseded by another row. When unsure, leave it alone.
Brew rows of kind lib (no bins) are usually dependencies of other formulae; leave them unless clearly drift.
Skip rows that need no change. Give each action a one-line reason that cites the evidence (status, uses, last, also). The summary is 1-3 sentences.'#

def manage-candidates [tools: list<record>]: nothing -> list<record> {
    $tools | where {|t|
        $t.source in $MANAGEABLE and (
            $t.overlap != "" or $t.status in [missing unused shadowed rare] or $t.managed == "drift"
        )
    }
}

# Actions valid for a row; the agent may only pick from these.
def allowed-actions [row: record]: nothing -> list<string> {
    let declarable = $row.source in $DECLARABLE
    [
        (if $declarable and not $row.installed { "install" })
        (if $row.installed { "uninstall" })
        (if $declarable and $row.installed and $row.managed == "drift" { "declare" })
        (if $declarable and $row.managed == "declared" { "undeclare" })
    ] | compact
}

def manage-prompt [rows: list<record>]: nothing -> string {
    let data = $rows | each {|t| {
        source: $t.source name: $t.name status: $t.status kind: $t.kind managed: $t.managed
        uses: $t.uses zsh: $t.zsh nu: $t.nu bash: $t.bash last: (fmt-date $t.last)
        also: $t.overlap bins: ($t.bins | first 8) desc: ($t.desc | str substring 0..79)
        allowed: (allowed-actions $t)
    } }
    [
        $MANAGE_BRIEF
        $"Today is (date now | format date '%Y-%m-%d'). Rows as JSON:"
        ($data | to json -r)
        "Reply with only a JSON object matching this JSON Schema, with no prose and no code fences:"
        ($PLAN_SCHEMA | to json -r)
    ] | str join "\n\n"
}

# The outermost {...} in an agent's text reply.
def json-in [text: string]: nothing -> any {
    let a = $text | str index-of -g "{"
    let b = $text | str index-of -g -e "}"
    if $a < 0 or $b < $a { error make {msg: $"agent reply has no JSON object: ($text | str substring -g 0..199)"} }
    $text | str substring -g $a..$b | from json
}

def ask-agent [agent: string, model: any, prompt: string]: nothing -> record {
    let m = if $model == null { [] } else { ["--model" $model] }
    match $agent {
        "claude" => {
            if not (has-cmd claude) { error make {msg: "claude CLI not found: install it or pass --agent omp"} }
            # Read-only tools stay available; StructuredOutput enforces the schema.
            let r = $prompt | ^claude -p --no-session-persistence --output-format json --json-schema ($PLAN_SCHEMA | to json -r) --disallowedTools "Bash,Edit,Write,NotebookEdit,WebFetch,WebSearch,Task" ...$m | complete
            let out = try { $r.stdout | from json } catch { error make {msg: $"claude failed \(exit ($r.exit_code)\): ($r.stderr | str trim)"} }
            if ($out.is_error? | default false) { error make {msg: $"claude: ($out.result? | default 'error'). Run `claude /login`, or pass --agent omp"} }
            $out.structured_output? | default (json-in ($out.result? | default ""))
        }
        "omp" => {
            if not (has-cmd omp) { error make {msg: "omp CLI not found: install it or pass --agent claude"} }
            let r = $prompt | ^omp -p --no-tools --no-session ...$m | complete
            if $r.exit_code != 0 { error make {msg: $"omp failed \(exit ($r.exit_code)\): ($r.stderr | str trim)"} }
            json-in $r.stdout
        }
        _ => (error make {msg: $"unknown agent '($agent)': use claude or omp"})
    }
}

def decl-file [repo: string, source: string]: nothing -> string {
    match $source {
        "brew" | "cask" => ($repo | path join config brew Brewfile)
        "cargo" => ($repo | path join config cargo liner.toml)
        "node" => ($repo | path join config node package.json)
        "uv" => ($repo | path join config uv tools.txt)
        _ => ""
    }
}

# What an action does, as data: {run: argv} or {edit: declare|undeclare, file}.
def manage-steps [repo: string, row: record, action: string]: nothing -> list<record> {
    let n = $row.name
    let pkg = $row.pkg? | default $n
    let file = decl-file $repo $row.source
    match $action {
        "declare" | "undeclare" => [{edit: $action, file: $file}]
        "install" => [{run: (match $row.source {
            "brew" => ["brew" "install" $pkg]
            "cask" => ["brew" "install" "--cask" $pkg]
            "cargo" => ["cargo" "install" "--locked" $n]
            "node" => ["bun" "add" "-g" $n]
            "uv" => ["uv" "tool" "install" $pkg]
        })}]
        "uninstall" => {
            let run = match $row.source {
                "brew" => ["brew" "uninstall" $n]
                "cask" => ["brew" "uninstall" "--cask" $n]
                "mise" => (if $row.managed == "global" { ["mise" "unuse" "-g" $n] } else { ["mise" "uninstall" "--all" $n] })
                "cargo" => ["cargo" "uninstall" $n]
                "node" => ["bun" "remove" "-g" $n]
                "uv" => ["uv" "tool" "uninstall" $n]
            }
            # Otherwise the next `update` reinstalls it.
            let undeclare = if $row.managed == "declared" { [{edit: "undeclare", file: $file}] } else { [] }
            [{run: $run}] ++ $undeclare
        }
    }
}

def step-text [repo: string, s: record]: nothing -> string {
    if $s.run? != null { $s.run | str join " " } else { $"($s.edit) in ($s.file | path relative-to $repo)" }
}

def save-lines [file: string, lines: list<string>]: nothing -> nothing {
    $lines | str join "\n" | $in + "\n" | save -f $file
}

def edit-brewfile [op: string, kind: string, name: string, pkg: string, note: string, file: string]: nothing -> nothing {
    let lines = open --raw $file | lines
    if $op == "undeclare" {
        save-lines $file ($lines | where {|l|
            let m = $l | parse -r '^\s*(?<t>brew|cask)\s+"(?<full>[^"]+)"' | get -o 0
            $m == null or $m.t != $kind or ($m.full | split row "/" | last) != $name
        })
        return
    }
    let parts = $pkg | split row "/"
    let tap = if ($parts | length) == 3 { $"tap \"($parts.0)/($parts.1)\"" } else { null }
    let tapped = $tap == null or ($lines | any {|l| $l | str trim | str lowercase | str starts-with ($tap | str lowercase) })
    let entry = $"($kind) \"($pkg)\""
    let entry = if $note == "" { $entry } else { $"($entry | fill -w 40) # ($note)" }
    save-lines $file ($lines | append (if $tapped { [] } else { [$tap] }) | append $entry)
}

def edit-liner [op: string, name: string, file: string]: nothing -> nothing {
    let lines = open --raw $file | lines
    let start = $lines | enumerate | where {|e| ($e.item | str trim) == "[packages]" } | get -o 0.index
    if $start == null { error make {msg: $"no [packages] table in ($file)"} }
    let end = $lines | enumerate | skip ($start + 1) | where {|e| $e.item | str trim | str starts-with "[" } | get -o 0.index | default ($lines | length)
    if $op == "undeclare" {
        save-lines $file ($lines | enumerate | where {|e|
            $e.index <= $start or $e.index >= $end or ($e.item | parse -r '^\s*"?(?<k>[^"=\s]+)"?\s*=' | get -o 0.k) != $name
        } | get item)
        return
    }
    mut at = $end
    while $at > $start + 1 and ($lines | get ($at - 1) | str trim) == "" { $at -= 1 }
    save-lines $file ($lines | insert $at $"($name) = \"*\"")
}

def edit-declaration [op: string, row: record, file: string]: nothing -> nothing {
    let n = $row.name
    let pkg = $row.pkg? | default $n
    let note = $row.desc | str replace -a -r '\s+' ' ' | str trim | str substring 0..59
    match $row.source {
        "brew" | "cask" => (edit-brewfile $op $row.source $n $pkg $note $file)
        "cargo" => (edit-liner $op $n $file)
        "node" => {
            let j = open $file
            let deps = if $op == "declare" { $j.dependencies | upsert $n "*" | sort } else { $j.dependencies | reject -o $n }
            $j | update dependencies $deps | to json -i 2 | $in + "\n" | save -f $file
        }
        "uv" => {
            let lines = open --raw $file | lines
            if $op == "declare" {
                save-lines $file ($lines | append (if $note == "" { $pkg } else { $"($pkg)   # ($note)" }))
            } else {
                save-lines $file ($lines | where {|l|
                    ($l | str replace -r '#.*' '' | str trim | split row -r '\s+' | first | str replace -r '[\[=<>~!;].*' '') != $n
                })
            }
        }
    }
}

def run-step [row: record, s: record]: nothing -> nothing {
    if $s.run? != null {
        run-external ($s.run | first) ...($s.run | skip 1)
    } else {
        edit-declaration $s.edit $row $s.file
    }
}

def print-action [repo: string, i: int, total: int, a: record]: nothing -> nothing {
    let facts = [(paint $a.row.status) $"($a.row.uses) uses" $a.row.overlap] | where $it != "" | str join " · "
    print $"(ansi white_bold)[($i + 1)/($total)](ansi reset) (paint-action $a.action) ($a.source)/($a.name)  ($facts)"
    print $"   ($a.reason)"
    for s in $a.steps { print $"   (ansi dark_gray)→ (step-text $repo $s)(ansi reset)" }
}

def paint-action [a: string]: nothing -> string {
    let c = match $a { "install" => "green", "uninstall" => "red", "declare" => "cyan", _ => "yellow" }
    $"(ansi $c)($a)(ansi reset)"
}

# AI-managed installs: an agent reviews duplicates (`also`/shadowed), status and
# drift across brew/cask/mise/cargo/node/uv and proposes install/uninstall/
# declare/undeclare. Plan only, unless --apply (confirms each action).
def "main manage" [
    --apply                          # run the plan, confirming each action
    --agent (-a): string = "claude"  # claude|omp
    --model (-m): string             # model passed to the agent
    --source (-s): string            # only review rows from brew|cask|mise|cargo|node|uv
    --json                           # plan as JSON (never applies)
] {
    let d = load
    let repo = $d.repo
    let rows = manage-candidates $d.tools | where {|r| $source == null or $r.source == $source }
    if ($rows | is-empty) { print "nothing to manage"; return }
    print -e $"(ansi dark_gray)asking ($agent) about ($rows | length) rows…(ansi reset)"
    let reply = ask-agent $agent $model (manage-prompt $rows)
    let checked = $reply.actions? | default [] | each {|p| {
        source: ($p.source? | default "") name: ($p.name? | default "")
        action: ($p.action? | default "") reason: ($p.reason? | default "")
    } } | uniq-by source name | each {|p|
        let row = $rows | where {|r| $r.source == $p.source and $r.name == $p.name } | get -o 0
        let why = if $row == null { "not a reviewed row" } else if $p.action not-in (allowed-actions $row) { $"not allowed \(allowed: ((allowed-actions $row) | str join ',')\)" } else { null }
        $p | insert row $row | insert rejected $why
    }
    let plan = $checked | where rejected == null | each {|p| $p | insert steps (manage-steps $repo $p.row $p.action) }
    let rejected = $checked | where rejected != null | reject row
    let summary = $reply.summary? | default ""

    if $json {
        return ({
            agent: $agent reviewed: ($rows | length) summary: $summary
            actions: ($plan | reject row rejected)
            rejected: $rejected
        } | to json)
    }

    print $"(ansi white_bold)toolbelt manage(ansi reset) (ansi dark_gray)· ($agent) · ($rows | length) rows reviewed · (date now | format date '%Y-%m-%d %H:%M')(ansi reset)"
    if $summary != "" { print $summary }
    if ($rejected | is-not-empty) {
        print $"(ansi dark_gray)ignored agent picks: ($rejected | each {|r| $'($r.action) ($r.source)/($r.name): ($r.rejected)' } | str join '; ')(ansi reset)"
    }
    if not $apply {
        header $"PLAN — ($plan | length) actions"
        if ($plan | is-empty) { print "no changes proposed" }
        for e in ($plan | enumerate) { print-action $repo $e.index ($plan | length) $e.item }
        print $"(ansi dark_gray)nothing changed · toolbelt manage --apply re-plans, then confirms each action(ansi reset)"
        return
    }
    if ($plan | is-empty) { print "no changes proposed"; return }

    header "APPLY"
    mut all = false
    mut log = []
    for e in ($plan | enumerate) {
        let a = $e.item
        print-action $repo $e.index ($plan | length) $a
        let answer = if $all { "y" } else { input "   apply? [y]es [n]o [a]ll [q]uit: " | str trim | str lowercase }
        if $answer == "q" { break }
        if $answer == "a" { $all = true }
        let result = if $answer in [y a] {
            try { for s in $a.steps { run-step $a.row $s }; "done" } catch {|err| $"failed: ($err.msg)" }
        } else { "skipped" }
        $log = $log | append {action: $a.action, tool: $"($a.source)/($a.name)", result: $result, edits: ($a.steps | any {|s| $s.edit? != null })}
    }
    header "RESULT"
    print ($log | reject edits | table -i false)
    if ($log | any {|l| $l.result == "done" and $l.edits }) { print $"(ansi dark_gray)declarations edited, review: git -C ($repo) diff config/(ansi reset)" }
}

# ── governance ───────────────────────────────────────────────────────────────
# `toolbelt govern`: read-only security & governance audit. Nothing is changed:
# kubectl only runs get/version/config view, gcloud only list/describe.
#
# Clusters: every kube context is scanned. A management cluster is detected by
# what it runs (Crossplane, CAPI) or by having children. Children are found
# by every mechanism that creates or targets one: CAPI Clusters, Crossplane
# cluster MRs (GKE/EKS/AKS/…) and kubernetes/helm ProviderConfigs, vcluster,
# Flux remote kubeConfig, Argo CD cluster secrets. A child is audited through
# a matching local context, else through its kubeconfig secret (decoded into a
# 0600 file in a private temp dir, removed afterwards), recursively down to
# MAX_DEPTH. Children that can't be reached are reported as unaudited.

const SEVERITIES = [crit high med low info ok]
const GOVERN_SCOPES = [shells gcp mcp agents clusters]
const SECRET_VALUE_RE = '(ghp_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}|gh[osur]_[A-Za-z0-9]{30,}|glpat-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{35}|sk-(?:ant-|proj-)?[A-Za-z0-9_-]{32,}|xox[abpr]-[A-Za-z0-9-]{10,}|ya29\.[A-Za-z0-9_-]{20,}|-----BEGIN [A-Z ]*PRIVATE KEY)'
const SECRET_KEY_RE = '(?i)(token|secret|passw(?:or)?d|api_?key|private_?key|credential|authorization)'
const SECRET_FLAG_RE = '(?i)--(?:password|passwd|token|api-key|secret)[= ]+[^\s$-]'
const AGENT_YOLO_RE = '--dangerously-skip-permissions|--dangerously-bypass-approvals-and-sandbox|--yolo\b|--full-auto|bypassPermissions|--approval-mode[= ]yolo'
const PKG_RUNNERS = [npx bunx pnpx uvx]
const SYSTEM_NS = [kube-system kube-public kube-node-lease]
const MAX_DEPTH = 3

def finding [scope: string, target: string, check: string, sev: string, detail: string = ""]: nothing -> record {
    {scope: $scope, target: $target, check: $check, sev: $sev, detail: $detail}
}

def tilde [p: string]: nothing -> string { $p | str replace $env.HOME "~" }

def slurp [p: string]: nothing -> string { open --raw $p | into string }

def short-list [n: int = 6]: list<any> -> string {
    let xs = $in
    let more = ($xs | length) - $n
    ($xs | first $n | str join " ") + (if $more > 0 { $" …+($more)" } else { "" })
}

# Unix mode (rwxr-x---) of a file or dir, symlinks followed; "" if absent.
def fmode [p: string]: nothing -> string {
    let x = $p | path expand
    if not ($x | path exists) { return "" }
    ls -lD $x | get -o 0.mode | default ""
}
def group-other-any [m: string]: nothing -> bool { ($m | str length) == 9 and (($m | str substring 3..) =~ '[rwx]') }
def group-write [m: string]: nothing -> bool { ($m | str length) == 9 and ($m | str substring 4..4) == "w" }
def other-write [m: string]: nothing -> bool { ($m | str length) == 9 and ($m | str substring 7..7) == "w" }

# Token families found in text, as 4-char prefixes: never the secret itself.
def secret-kinds [text: string]: nothing -> list<string> {
    $text | parse -r $SECRET_VALUE_RE | each {|m| $"($m.capture0 | str substring 0..3)…" } | uniq
}

# ── governance: shells ──

def govern-shells [repo: string, inv: list<record>]: nothing -> list<record> {
    let home = $env.HOME
    let zdot = $env.ZDOTDIR? | default ($home | path join .config zsh)
    let fixed_logs = [{shell: zsh, path: (zsh-log-path)} {shell: nu, path: (nu-db-path)} {shell: nu, path: (nu-txt-path)} {shell: bash, path: (bash-history-path)}]
    let history = zsh-history-paths | each {|p| {shell: zsh, path: $p} } | append $fixed_logs | where {|h| $h.path | path exists }
    let hist_f = $history | each {|h|
        let m = fmode $h.path
        if (group-other-any $m) {
            finding shells $h.shell "history private" med $"(tilde $h.path) is ($m): chmod 600"
        } else { finding shells $h.shell "history private" ok (tilde $h.path) }
    }
    let shell_of = {zh: zsh, zl: zsh, nd: nu, nt: nu, bh: bash}
    let leak_f = [zsh nu bash] | each {|sh|
        let lines = $inv | where {|i| ($shell_of | get $i.src) == $sh } | each {|i| $i.line } | uniq
        let strong = $lines | where {|l| $l =~ $SECRET_VALUE_RE }
        let weak = $lines | where {|l| ($l =~ $SECRET_FLAG_RE) and not ($l =~ $SECRET_VALUE_RE) }
        [
            (if ($strong | is-not-empty) {
                let kinds = $strong | each {|l| secret-kinds $l } | flatten | uniq | str join " "
                finding shells $sh "credentials in history" high $"($strong | length) lines hold literal tokens \(($kinds)\): scrub them from history and rotate"
            })
            (if ($weak | is-not-empty) {
                finding shells $sh "credentials in history" med $"($weak | length) lines pass --password/--token/--secret on the command line"
            })
        ] | compact
    } | flatten
    let rc_f = [
        {shell: zsh, path: ($zdot | path join .zshrc)}
        {shell: zsh, path: ($home | path join .zshenv)}
        {shell: zsh, path: ($zdot | path join generated.zsh)}
        {shell: nu, path: ($home | path join .config nushell config.nu)}
        {shell: nu, path: ($home | path join .config nushell env.nu)}
        {shell: nu, path: ($home | path join .config nushell generated.nu)}
        {shell: bash, path: ($home | path join .bashrc)}
        {shell: bash, path: ($home | path join .bash_profile)}
    ] | where {|r| $r.path | path exists } | each {|r|
        let m = fmode $r.path
        if (group-write $m) or (other-write $m) {
            finding shells $r.shell "startup files tamper-proof" high $"(tilde $r.path) is ($m): whoever can write it runs code in every shell"
        } else { finding shells $r.shell "startup files tamper-proof" ok (tilde $r.path) }
    }
    let path_dirs = $env.PATH | uniq
    # A dir (or, for a missing entry, its nearest existing ancestor) that other
    # accounts can write lets them put binaries ahead of yours.
    let writable_by = {|d|
        let m = fmode $d
        if (other-write $m) { "any user" } else if (group-write $m) { $"group (ls -lD ($d | path expand) | get 0.group)" }
    }
    let path_f = $path_dirs | each {|d|
        if not ($d | str starts-with "/") {
            return (finding shells PATH "PATH hygiene" high $"relative entry '($d)': commands resolve from the current directory")
        }
        mut base = $d
        while not ($base | path exists) { $base = $base | path dirname }
        let who = do $writable_by $base
        if $who == null { return null }
        let sev = if $who == "any user" { "high" } else { "med" }
        let what = if $base == $d { $"(tilde $d) is writable by ($who)" } else { $"(tilde $d) is missing and ($who) can create it under (tilde $base)" }
        finding shells PATH "PATH hygiene" $sev $"($what): they can plant binaries on your PATH"
    } | compact
    let home_missing = $path_dirs | where {|d| ($d | str starts-with $home) and not ($d | path exists) }
    let missing_f = if ($home_missing | is-empty) { [] } else {
        [(finding shells PATH "PATH hygiene" low $"stale entries: ($home_missing | each {|d| tilde $d } | short-list)")]
    }
    let cfg_path = $repo | path join config shell config.toml
    let cfg = open $cfg_path
    let env_f = $cfg.environment? | default {} | transpose k v
        | where {|e| let v = $e.v | into string; ($e.k =~ $SECRET_KEY_RE) and not ($v =~ '^\$|\$\{|^$') }
        | each {|e| finding shells config.toml "no committed secrets" crit $"[environment] ($e.k) holds a literal value: load it from a secret store" }
    let raw_kinds = secret-kinds (slurp $cfg_path)
    let raw_f = if ($raw_kinds | is-empty) { [] } else {
        [(finding shells config.toml "no committed secrets" crit $"literal tokens in config/shell/config.toml: ($raw_kinds | str join ' ')")]
    }
    let wrapped = $cfg.aliases? | default {} | transpose name exp
        | where {|a| $a.exp | str starts-with "sfw " } | each {|a| $a.exp | split row " " | get 1 } | uniq
    let unwrapped = [npm npx pnpx bunx uvx yarn pip pip3] | where {|b| $b not-in $wrapped and (has-cmd $b) }
    let sfw_f = [
        (if ($wrapped | is-not-empty) {
            if (has-cmd sfw) {
                finding shells aliases "supply-chain firewall" ok $"sfw guards ($wrapped | str join ' ')"
            } else {
                finding shells aliases "supply-chain firewall" high $"aliases route ($wrapped | str join ' ') through sfw, but sfw is not installed"
            }
        })
        (if ($unwrapped | is-not-empty) {
            finding shells aliases "supply-chain firewall" low $"installers not routed through sfw: ($unwrapped | str join ' ')"
        })
    ] | compact
    $hist_f ++ $leak_f ++ $rc_f ++ $path_f ++ $missing_f ++ $env_f ++ $raw_f ++ $sfw_f
}

# ── governance: gcp ──

def --wrapped gcloud-json [...args: string]: nothing -> any {
    let r = ^gcloud ...$args --format=json --quiet | complete
    if $r.exit_code != 0 { return null }
    try { $r.stdout | from json } catch { null }
}

# {sa, id} when the file is a service-account key, else null.
def sa-key-info [p: string]: nothing -> any {
    let j = try { slurp $p | from json } catch { null }
    if ($j == null) or (($j | describe) !~ '^record') { return null }
    if ($j.type? != "service_account") or ($j.private_key? | is-empty) { return null }
    {sa: ($j.client_email? | default "?"), id: ($j.private_key_id? | default "" | str substring 0..7)}
}

def govern-gcp [repo: string]: nothing -> list<record> {
    if not (has-cmd gcloud) { return [(finding gcp gcloud "gcloud installed" info "gcloud not on PATH: skipped")] }
    let cfg_dir = $env.CLOUDSDK_CONFIG? | default ($env.HOME | path join .config gcloud)
    let configs = gcloud-json config configurations list | default []
    let conf_f = $configs | each {|c|
        let acct = $c.properties?.core?.account? | default ""
        let proj = $c.properties?.core?.project? | default ""
        let imp = $c.properties?.auth?.impersonate_service_account? | default ""
        let t = if $c.is_active { $"config ($c.name) \(active\)" } else { $"config ($c.name)" }
        [
            (if $acct =~ '@(gmail|googlemail)\.com$' { finding gcp $t "org identities only" med $"($acct) is a consumer account: no org policy, SSO or audit trail" })
            (if $acct =~ 'iam\.gserviceaccount\.com$' { finding gcp $t "no service-account logins" high $"($acct) is logged in directly: impersonate it instead \(auth/impersonate_service_account\)" })
            (if ($imp | is-not-empty) { finding gcp $t "short-lived credentials" ok $"impersonates ($imp)" })
            (if ($acct | is-empty) { finding gcp $t "identity set" low "no core/account" })
            (if ($proj | is-empty) { finding gcp $t "project pinned" low "no core/project: commands act on whatever --project resolves to" })
        ] | compact
    } | flatten
    let used = $configs | each {|c| $c.properties?.core?.account? } | compact
    let creds = gcloud-json auth list | default []
    let stale = $creds | where {|a| $a.account not-in $used }
    let activated = $creds | where {|a| $a.account =~ 'iam\.gserviceaccount\.com$' }
    let cred_f = [
        (if ($stale | is-not-empty) {
            finding gcp "credential store" "no stale credentials" low $"cached but unused by any config: ($stale | each {|a| $a.account } | str join ' '): gcloud auth revoke"
        })
        (if ($activated | is-not-empty) {
            finding gcp "credential store" "no long-lived SA keys" high $"service-account keys activated in gcloud: ($activated | each {|a| $a.account } | str join ' ')"
        })
    ] | compact
    let adc = $cfg_dir | path join application_default_credentials.json
    let adc_f = if not ($adc | path exists) { [] } else {
        let t = try { slurp $adc | from json | get -o type } catch { null }
        let m = fmode $adc
        [
            (match $t {
                "service_account" => (finding gcp ADC "keyless ADC" high $"(tilde $adc) is a service-account key: use `gcloud auth application-default login --impersonate-service-account`")
                "authorized_user" => (finding gcp ADC "keyless ADC" info $"user refresh token in (tilde $adc)")
                "impersonated_service_account" | "external_account" => (finding gcp ADC "keyless ADC" ok $t)
                _ => null
            })
            (if (group-other-any $m) { finding gcp ADC "credential files private" high $"(tilde $adc) is ($m): chmod 600" })
        ] | compact
    }
    # Places service-account keys end up: devkit's ESO creds, GOOGLE_APPLICATION_CREDENTIALS,
    # the repo's tmp/, gcloud's config dir and ~/Downloads (console key downloads).
    let devkit_cfg = try { open ($repo | path join devkit.toml) } catch { {} }
    let named = [($devkit_cfg.external_secrets?.gcp_credentials?) ($env.GOOGLE_APPLICATION_CREDENTIALS?)]
        | compact | where $it != "" | each {|p| $p | path expand }
    let globbed = [$"($repo)/tmp/**/*.json" ($cfg_dir | path join "*.json") ($env.HOME | path join Downloads "*.json")]
        | each {|g| try { glob $g } catch { [] } } | flatten
    let repo_x = $repo | path expand
    let key_f = $named ++ $globbed | uniq | where {|p| $p | path exists } | each {|p|
        let k = sa-key-info $p
        if $k == null { return [] }
        let age = ((date now) - (ls -lD $p | get 0.modified)) / 1day | math floor
        let m = fmode $p
        let in_repo = $p | str starts-with $repo_x
        let tracked = $in_repo and ((^git -C $repo_x ls-files --error-unmatch $p | complete).exit_code == 0)
        let ignored = $in_repo and ((^git -C $repo_x check-ignore -q $p | complete).exit_code == 0)
        [
            (finding gcp (tilde $p) "no long-lived SA keys" high $"key ($k.id)… of ($k.sa), ($age)d old: prefer Workload Identity or impersonation")
            (if (group-other-any $m) { finding gcp (tilde $p) "credential files private" crit $"SA key is ($m): chmod 600" })
            (if $tracked {
                finding gcp (tilde $p) "no committed secrets" crit "SA key is tracked by git: rotate it"
            } else if $in_repo and not $ignored {
                finding gcp (tilde $p) "no committed secrets" high "SA key inside the repo is not git-ignored"
            })
        ] | compact
    } | flatten
    $conf_f ++ $cred_f ++ $adc_f ++ $key_f
}

# ── governance: mcp ──

# Client config registry, straight from mcp.nu so the two never drift.
def mcp-targets [repo: string]: nothing -> record {
    let src = $repo | path join config scripts mcp.nu
    let r = ^nu --no-config-file -c $"use '($src)' TARGETS; $TARGETS | to json" | complete
    if $r.exit_code != 0 { return {} }
    $r.stdout | from json
}

# One shape for every client: flat (claude/cursor/codex/gemini) or zed's nested command.
def mcp-norm [d: any]: nothing -> record {
    if ($d | describe) !~ '^record' { return {command: null, args: [], env: {}, url: null, headers: {}} }
    let c = $d.command?
    let nested = ($c | describe) =~ '^record'
    {
        command: (if $nested { $c.path? } else { $c })
        args: ((if $nested { $c.args? } else { $d.args? }) | default [] | each {|a| $a | into string })
        env: ((if $nested { $c.env? } else { $d.env? }) | default {})
        url: ($d.url? | default $d.serverUrl? | default $d.httpUrl?)
        headers: ($d.headers? | default {})
    }
}

# Package a runner (npx/bunx/uvx) fetches, from its argv.
def mcp-package [args: list<string>]: nothing -> any {
    let explicit = $args | where {|a| $a =~ '^--(package|from)=' } | get -o 0
    if $explicit != null { return ($explicit | str replace -r '^--[a-z]+=' '') }
    let i = $args | enumerate | where {|e| $e.item in [-p --package --from] } | get -o 0.index
    if $i != null { return ($args | get -o ($i + 1)) }
    $args | where {|a| not ($a | str starts-with "-") } | get -o 0
}

def mcp-server-findings [origin: string, name: string, d: any, governed: any, source_of_truth: bool]: nothing -> list<record> {
    let n = mcp-norm $d
    let t = $"($origin) › ($name)"
    let cmd = $n.command | default "" | path basename
    let run_args = if $cmd in $PKG_RUNNERS { $n.args } else if ($cmd in [bun pnpm]) and (($n.args | get -o 0) in [x dlx]) {
        $n.args | skip 1
    } else if $cmd == "uv" and (($n.args | first 2) == [tool run]) { $n.args | skip 2 } else { null }
    let pkg = if $run_args == null { null } else { mcp-package $run_args }
    let secrets = ($n.env | transpose k v) ++ ($n.headers | transpose k v)
        | where {|e| let v = $e.v | into string; ($v != "") and not ($v =~ '\$\{') and (($e.k =~ $SECRET_KEY_RE) or ($v =~ $SECRET_VALUE_RE)) }
    let keys = $secrets | each {|e| $e.k } | str join ","
    [
        (if ($governed != null) and ($name not-in $governed) { finding mcp $t "declared in servers.json" med "not in config/mcp/servers.json: no review, no opt-in gate" })
        (if ($pkg != null) and not ($pkg =~ '^(@[^/]+/)?[^@=<>]+(@|==)v?\d') { finding mcp $t "pinned packages" med $"`($cmd) ($pkg)` runs the newest release on every launch: pin a version" })
        (if ($n.url != null) and ($n.url =~ '^http://') and not ($n.url =~ '^http://(localhost|127\.0\.0\.1|\[::1\])') { finding mcp $t "encrypted transport" high $"($n.url) is plaintext" })
        (if ($secrets | is-not-empty) {
            if $source_of_truth {
                finding mcp $t "no literal secrets" crit $"literal ($keys) in the source of truth: reference \${VAR} instead"
            } else {
                finding mcp $t "no literal secrets" med $"($keys) materialized in plaintext"
            }
        })
        (if ($d.disabled? == true) or ($d._enabled? == false) { finding mcp $t "opt-in" info "disabled / opt-in" })
    ] | compact
}

def govern-mcp [repo: string]: nothing -> list<record> {
    let home = $env.HOME
    let declared = try { open ($repo | path join config mcp servers.json) | get -o mcpServers | default {} } catch { {} }
    let governed = $declared | columns
    let own_f = $declared | transpose name d | each {|s| mcp-server-findings "servers.json" $s.name $s.d null true } | flatten
    let claude_json = try { open ($home | path join .claude.json) } catch { {} }
    let project_dirs = $claude_json.projects? | default {} | columns | append $repo | uniq
    let targets = mcp-targets $repo | transpose target d
    let absolute = {|p| ($p | str starts-with "~") or ($p | str starts-with "/") }
    let fixed = $targets | where {|t| do $absolute $t.d.path } | each {|t|
        {origin: $t.target, path: ($t.d.path | path expand), format: $t.d.format, key: $t.d.key}
    }
    let per_project = $targets | where {|t| not (do $absolute $t.d.path) } | each {|t|
        $project_dirs | each {|dir| {origin: $"($t.target) (tilde $dir)", path: ($dir | path join $t.d.path), format: $t.d.format, key: $t.d.key} }
    } | flatten
    let files = $fixed ++ $per_project ++ [{origin: "claude-code user", path: ($home | path join .claude.json), format: json, key: mcpServers}]
        | where {|f| $f.path | path exists }
    let file_f = $files | each {|f|
        let raw = slurp $f.path
        let data = try { if $f.format == "toml" { $raw | from toml } else { $raw | from json } } catch { null }
        if $data == null { return [(finding mcp (tilde $f.path) "parsable config" low "unparseable: not audited")] }
        let servers = $data | get -o $f.key | default {}
        let servers = if ($servers | describe) =~ '^record' { $servers } else { {} }
        let m = fmode $f.path
        let server_f = $servers | transpose name d | each {|s| mcp-server-findings $f.origin $s.name $s.d $governed false } | flatten
        let perm_f = if ($raw =~ $SECRET_VALUE_RE) and (group-other-any $m) {
            [(finding mcp (tilde $f.path) "credential files private" high $"holds live tokens and is ($m): chmod 600")]
        } else { [] }
        $server_f ++ $perm_f
    } | flatten
    # Claude Code local scope: servers stored per project inside ~/.claude.json.
    let local_f = $claude_json.projects? | default {} | transpose dir p | each {|x|
        $x.p.mcpServers? | default {} | transpose name d
        | each {|s| mcp-server-findings $"claude-code local (tilde $x.dir)" $s.name $s.d $governed false } | flatten
    } | flatten
    let all = $own_f ++ $file_f ++ $local_f
    let summary = finding mcp "servers.json" "inventory" ok $"($governed | length) declared servers; ($files | length) client configs audited"
    $all ++ [$summary]
}

# ── governance: agents ──

def govern-agents [repo: string, inv: list<record>]: nothing -> list<record> {
    let home = $env.HOME
    let cdir = $home | path join .claude
    let managed = ["/Library/Application Support/ClaudeCode/managed-settings.json" "/etc/claude-code/managed-settings.json"]
        | where {|p| $p | path exists }
    let settings = [($cdir | path join settings.json) ($cdir | path join settings.local.json)] ++ $managed
        | where {|p| $p | path exists } | each {|p| try { slurp $p | from json } catch { {} } }
    let perms = $settings | each {|s| $s.permissions? | default {} }
    let mode = $perms | each {|p| $p.defaultMode? } | compact | reverse | get -o 0
    let allow = $perms | each {|p| $p.allow? | default [] } | flatten
    let deny = $perms | each {|p| $p.deny? | default [] } | flatten
    let broad = $allow | where {|r| ($r in [Bash "Bash(*)" "Bash(*:*)" "*" Write Edit WebFetch]) or ($r =~ '^(Write|Edit)\(\*\*?\)$') }
    let env_secret = $settings | each {|s| $s.env? | default {} | transpose k v } | flatten
        | where {|e| let v = $e.v | into string; ($v != "") and not ($v =~ '\$\{') and (($e.k =~ $SECRET_KEY_RE) or ($v =~ $SECRET_VALUE_RE)) }
    let claude_state = try { open ($home | path join .claude.json) } catch { {} }
    let bypass_accepted = ($settings | any {|s| $s.skipDangerousModePermissionPrompt? == true }) or ($claude_state.bypassPermissionsModeAccepted? == true)
    let claude_f = if not (($cdir | path exists) or (has-cmd claude)) { [] } else {
        [
            (if $mode == "bypassPermissions" { finding agents claude-code "human approval" high "permissions.defaultMode = bypassPermissions: every tool call runs unprompted" })
            (if $mode == "acceptEdits" { finding agents claude-code "human approval" low "defaultMode = acceptEdits: file writes are unprompted" })
            (if $bypass_accepted { finding agents claude-code "human approval" med "bypass-permissions mode has been accepted on this machine" })
            (if ($broad | is-not-empty) { finding agents claude-code "least-privilege tools" high $"allow-listed without scope: ($broad | str join ' ')" })
            (if ($deny | is-empty) { finding agents claude-code "deny rules" low 'no permissions.deny: nothing blocks Read(.env), Bash(curl:*), …' })
            (if ($managed | is-empty) {
                finding agents claude-code "managed policy" med "no managed-settings.json: every guardrail is user-editable"
            } else { finding agents claude-code "managed policy" ok ($managed | str join " ") })
            (if ($env_secret | is-not-empty) { finding agents claude-code "no literal secrets" high $"settings env holds ($env_secret | each {|e| $e.k } | str join ',')" })
        ] | compact
    }
    let hook_f = $settings | each {|s|
        $s.hooks? | default {} | values | flatten | each {|m| $m.hooks? | default [] } | flatten | each {|h| $h.command? } | compact
    } | flatten | uniq | each {|c|
        let words = $c | split row -r '\s+'
        let exe = if $words.0 in $RUNNERS { $words | get -o 1 | default $words.0 } else { $words.0 }
        let path = if ($exe | str contains "/") { $exe | path expand } else { which $exe | get -o 0.path | default "" }
        let m = if $path == "" { "" } else { fmode $path }
        if $m == "" {
            finding agents "claude-code hook" "hooks resolvable" med $"($c): binary missing, whatever lands at that path receives every prompt"
        } else if (group-write $m) or (other-write $m) {
            finding agents "claude-code hook" "hooks tamper-proof" high $"(tilde $path) is ($m) and runs on every tool call"
        } else {
            finding agents "claude-code hook" "hook inventory" info $"(tilde $c) receives every prompt and tool call"
        }
    }
    let plugin_f = $settings | each {|s|
        let markets = $s.extraKnownMarketplaces? | default {} | transpose name m | each {|x|
            let src = $x.m.source?.repo? | default ($x.m.source?.url? | default "?")
            finding agents "claude-code plugins" "vetted plugin sources" med $"third-party marketplace ($x.name) \(($src)\) ships unreviewed, unpinned code"
        }
        let on = $s.enabledPlugins? | default {} | transpose name on | where on == true | each {|p| $p.name }
        $markets ++ (if ($on | is-empty) { [] } else { [(finding agents "claude-code plugins" "plugin inventory" info ($on | str join " "))] })
    } | flatten
    let codex = try { open ($home | path join .codex config.toml) } catch { null }
    let codex_f = if $codex == null { [] } else {
        let trusted = $codex.projects? | default {} | transpose dir p | where {|x| $x.p.trust_level? == "trusted" } | each {|x| $x.dir }
        let roots = $trusted | where {|d| ($d | path expand) in [$home "/"] }
        [
            (if $codex.approval_policy? == "never" { finding agents codex "human approval" high "approval_policy = never" })
            (if $codex.sandbox_mode? == "danger-full-access" { finding agents codex "sandbox" high "sandbox_mode = danger-full-access: no filesystem or network sandbox" })
            (if ($roots | is-not-empty) { finding agents codex "trust scope" med $"trusted root ($roots | each {|d| tilde $d } | str join ' ') covers every repo beneath it" })
            (if ($trusted | is-not-empty) { finding agents codex "trust scope" info $"($trusted | length) trusted projects" })
        ] | compact
    }
    let gemini = try { slurp ($home | path join .gemini settings.json) | from json } catch { null }
    let gemini_f = if $gemini == null { [] } else {
        [
            (if ($gemini.tools?.autoAccept? == true) or (($gemini | to json -r) =~ '"yolo"') { finding agents gemini "human approval" high "auto-accept / yolo approval mode configured" })
            (if $gemini.security?.auth?.selectedType? == "oauth-personal" { finding agents gemini "org identities only" med "signed in with a personal Google account: no org data governance" })
        ] | compact
    }
    let cred_f = [
        {agent: codex, path: ($home | path join .codex auth.json)}
        {agent: gemini, path: ($home | path join .gemini oauth_creds.json)}
        {agent: claude-code, path: ($cdir | path join .credentials.json)}
    ] | where {|c| $c.path | path exists } | each {|c|
        let m = fmode $c.path
        if (group-other-any $m) {
            finding agents $c.agent "credential files private" high $"(tilde $c.path) is ($m): chmod 600"
        } else { finding agents $c.agent "credential files private" ok (tilde $c.path) }
    }
    let transcript_f = [
        {agent: claude-code, path: ($cdir | path join history.jsonl)}
        {agent: codex, path: ($home | path join .codex history.jsonl)}
    ] | where {|t| $t.path | path exists } | each {|t|
        let hits = slurp $t.path | lines | where {|l| $l =~ $SECRET_VALUE_RE }
        if ($hits | is-not-empty) {
            let kinds = $hits | each {|l| secret-kinds $l } | flatten | uniq | str join " "
            finding agents $t.agent "credentials in transcripts" high $"($hits | length) prompts in (tilde $t.path) hold tokens \(($kinds)\): rotate them"
        }
    } | compact
    let cfg = open ($repo | path join config shell config.toml)
    let fn_bodies = $cfg.functions? | default {} | transpose name f | each {|x| {name: $x.name, body: ($x.f.commands? | default [] | str join "; ")} }
    let launchers = $cfg.aliases? | default {} | transpose name body | append $fn_bodies
    let launcher_f = $launchers | where {|l| $l.body =~ $AGENT_YOLO_RE } | each {|l|
        finding agents $"alias ($l.name)" "human approval" high $"launches an agent with approvals off: ($l.body)"
    }
    let yolo_f = $inv | where {|i| $i.line =~ $AGENT_YOLO_RE } | each {|i| $i.line | split row -r '\s+' | first }
        | uniq -c | each {|g| finding agents $g.value "human approval" med $"launched with approvals/sandbox off ($g.count) times from the shell" }
    let devkit = try { open ($repo | path join devkit.toml) } catch { {} }
    let fleet = $devkit.fleet?
    let fleet_f = if $fleet == null { [] } else {
        let hub = $fleet.hub? | default ""
        [
            (if ($fleet.token? | default "") == "" { finding agents "devkit fleet" "authenticated agents" high 'fleet.token = "": the hub accepts reports from anyone who can reach it' })
            (if ($hub =~ '^http://') and not ($hub =~ '^http://(localhost|127\.0\.0\.1|\[::1\])') { finding agents "devkit fleet" "encrypted transport" med $"agents report to ($hub) in plaintext" })
            (finding agents "devkit fleet" "agent inventory" info $"hub ($hub), ($fleet.probes? | default [] | length) agentless probes")
        ] | compact
    }
    $claude_f ++ $hook_f ++ $plugin_f ++ $codex_f ++ $gemini_f ++ $cred_f ++ $transcript_f ++ $launcher_f ++ $yolo_f ++ $fleet_f
}

# ── governance: clusters ──

def --wrapped kube [k: list<string>, ...args: string]: nothing -> any {
    let r = ^kubectl ...$k --request-timeout=8s ...$args -o json | complete
    if $r.exit_code != 0 { return null }
    try { $r.stdout | from json } catch { null }
}

def kube-items [k: list<string>, resource: string]: nothing -> list<any> {
    kube $k get $resource -A | get -o items | default []
}

def label-of [meta: any, key: string]: nothing -> any {
    $meta.labels? | default {} | transpose k v | where k == $key | get -o 0.v
}

def ns-name [o: record]: nothing -> string { $"($o.metadata.namespace? | default '')/($o.metadata.name)" }

def floating-ref [ref: string]: nothing -> bool {
    not ($ref | str contains "@sha256:") and (($ref | str ends-with ":latest") or not ($ref | split row "/" | last | str contains ":"))
}

# name / kind / group / categories of every CRD, without shipping schemas through nu.
def crd-index [k: list<string>]: nothing -> list<record> {
    let r = ^kubectl ...$k --request-timeout=20s get crd --no-headers -o "custom-columns=N:.metadata.name,K:.spec.names.kind,C:.spec.names.categories" | complete
    if $r.exit_code != 0 { return [] }
    $r.stdout | lines | parse -r '^(?<name>\S+)\s+(?<kind>\S+)\s*(?<cat>.*)$' | each {|c| {
        name: $c.name
        kind: $c.kind
        group: ($c.name | str replace -r '^[^.]+\.' '')
        cats: ($c.cat | str replace -a -r '[\[\]"]' '' | split row -r '[,\s]+' | where {|x| $x not-in ["" "<none>"] })
    } }
}

# Clusters this one created or deploys into: {name ns via secret server state spec}.
def discover-children [k: list<string>, crds: list<record>]: nothing -> list<record> {
    let names = $crds | each {|c| $c.name }
    let capi = if "clusters.cluster.x-k8s.io" in $names {
        kube-items $k clusters.cluster.x-k8s.io | each {|c|
            let ep = $c.spec?.controlPlaneEndpoint?
            {
                name: $c.metadata.name, ns: $c.metadata.namespace, via: "capi"
                secret: {ns: $c.metadata.namespace, name: $"($c.metadata.name)-kubeconfig", key: "value"}
                server: (if ($ep.host? | is-empty) { null } else { $"https://($ep.host):($ep.port? | default 6443)" })
                state: ($c.status?.phase? | default "?"), spec: null
            }
        }
    } else { [] }
    let xp_kinds = $crds | where {|c|
        ("managed" in $c.cats) and (($c.kind in [KubernetesCluster ManagedCluster CivoKubernetes]) or (($c.kind == "Cluster") and ($c.group =~ '^(container\.gcp|eks\.aws|containerservice\.azure|kubernetes\.)')))
    }
    let crossplane = $xp_kinds | each {|c|
        kube-items $k $c.name | each {|m|
            let ref = $m.spec?.writeConnectionSecretToRef?
            let ep = $m.status?.atProvider?.endpoint?
            let ns = $m.metadata.namespace? | default ""
            {
                name: $m.metadata.name, ns: $ns, via: $"crossplane ($c.group | str replace -r '\.upbound\.io$' '')/($c.kind)"
                secret: (if $ref == null { null } else { {ns: ($ref.namespace? | default (if $ns == "" { "default" } else { $ns })), name: $ref.name, key: "kubeconfig"} })
                server: (if ($ep | is-empty) { null } else if ($ep | str starts-with "http") { $ep } else { $"https://($ep)" })
                state: $"Ready=($m.status?.conditions? | default [] | where {|x| $x.type? == 'Ready' } | get -o 0.status | default '?')"
                spec: (if $c.group =~ '^container\.gcp' { $m.status?.atProvider? } else { null })
            }
        }
    } | flatten
    let provider_configs = $crds | where {|c| ($c.name | str starts-with "providerconfigs.") and ($c.group =~ '^(kubernetes|helm)\.(m\.)?crossplane\.io$') }
        | each {|c|
            kube-items $k $c.name | where {|p| $p.spec?.credentials?.source? == "Secret" } | each {|p|
                let r = $p.spec.credentials.secretRef
                {
                    name: $p.metadata.name, ns: ($p.metadata.namespace? | default ""), via: $"crossplane ($c.group | str replace -r '\.crossplane\.io$' '')/ProviderConfig"
                    secret: {ns: ($r.namespace? | default ($p.metadata.namespace? | default "crossplane-system")), name: $r.name, key: ($r.key? | default "kubeconfig")}
                    server: null, state: "", spec: null
                }
            }
        } | flatten
    let vclusters = kube $k get statefulsets,deployments -A -l app=vcluster | get -o items | default [] | each {|w|
        let n = label-of $w.metadata release | default $w.metadata.name
        {
            name: $n, ns: $w.metadata.namespace, via: "vcluster"
            secret: {ns: $w.metadata.namespace, name: $"vc-($n)", key: "config"}
            server: null, state: $"($w.status?.readyReplicas? | default 0)/($w.spec?.replicas? | default 1) ready", spec: null
        }
    }
    let flux = [kustomizations.kustomize.toolkit.fluxcd.io helmreleases.helm.toolkit.fluxcd.io] | where {|r| $r in $names } | each {|r|
        kube-items $k $r | where {|x| $x.spec?.kubeConfig? != null } | each {|x|
            let s = $x.spec.kubeConfig.secretRef?
            let cm = $x.spec.kubeConfig.configMapRef?
            {
                name: ($s.name? | default ($cm.name? | default $x.metadata.name)), ns: $x.metadata.namespace, via: "flux kubeConfig"
                secret: (if $s == null { null } else { {ns: $x.metadata.namespace, name: $s.name, key: ($s.key? | default "value")} })
                server: null, state: "", spec: null
            }
        }
    } | flatten
    let argo = kube $k get secrets -A -l argocd.argoproj.io/secret-type=cluster | get -o items | default [] | each {|s|
        let d = $s.data? | default {}
        {
            name: (if ($d.name? | is-empty) { $s.metadata.name } else { $d.name | decode base64 | decode utf-8 })
            ns: $s.metadata.namespace, via: "argocd", secret: null
            server: (if ($d.server? | is-empty) { null } else { $d.server | decode base64 | decode utf-8 })
            state: "", spec: null
        }
    }
    $capi ++ $crossplane ++ $provider_configs ++ $vclusters ++ $flux ++ $argo | uniq-by via ns name
}

# Control-plane posture, normalized from `gcloud container clusters describe`.
def gke-from-gcloud [d: record]: nothing -> record {
    {
        wi: ($d.workloadIdentityConfig?.workloadPool? | is-not-empty)
        private_nodes: (($d.privateClusterConfig?.enablePrivateNodes? == true) or ($d.networkConfig?.defaultEnablePrivateNodes? == true))
        authorized_networks: ($d.masterAuthorizedNetworksConfig?.enabled? == true)
        legacy_abac: ($d.legacyAbac?.enabled? == true)
        shielded: ($d.shieldedNodes?.enabled? == true)
        binauthz: ($d.binaryAuthorization?.evaluationMode? | default "DISABLED")
        channel: ($d.releaseChannel?.channel? | default "UNSPECIFIED")
    }
}

def first-of [v: any]: nothing -> any { if ($v | describe) =~ '^list' { $v | get -o 0 } else { $v } }

# …and from a Crossplane container.gcp Cluster's status.atProvider (Upjet shape).
def gke-from-upbound [a: record]: nothing -> record {
    {
        wi: ((first-of $a.workloadIdentityConfig?).workloadPool? | is-not-empty)
        private_nodes: ((first-of $a.privateClusterConfig?).enablePrivateNodes? == true)
        authorized_networks: ((first-of $a.masterAuthorizedNetworksConfig?) != null)
        legacy_abac: ($a.enableLegacyAbac? == true)
        shielded: ($a.enableShieldedNodes? == true)
        binauthz: ((first-of $a.binaryAuthorization?).evaluationMode? | default "DISABLED")
        channel: ((first-of $a.releaseChannel?).channel? | default "UNSPECIFIED")
    }
}

def gke-findings [target: string, g: record]: nothing -> list<record> {
    let c = "gke control plane"
    let out = [
        (if $g.legacy_abac { finding clusters $target $c crit "legacy ABAC on: RBAC is bypassed" })
        (if not $g.wi { finding clusters $target $c high "Workload Identity off: pods act as the node service account" })
        (if not ($g.authorized_networks or $g.private_nodes) {
            finding clusters $target $c high "public control plane with no authorized networks"
        } else if not $g.authorized_networks { finding clusters $target $c med "control plane accepts any source IP" })
        (if not $g.private_nodes { finding clusters $target $c med "nodes have public IPs" })
        (if not $g.shielded { finding clusters $target $c med "shielded nodes off" })
        (if $g.binauthz == "DISABLED" { finding clusters $target $c low "Binary Authorization off: any image may run" })
        (if $g.channel == "UNSPECIFIED" { finding clusters $target $c low "no release channel: upgrades are manual" })
    ] | compact
    if ($out | is-empty) { [(finding clusters $target $c ok "hardened")] } else { $out }
}

def cluster-checks [k: list<string>, t: string, crds: list<record>]: nothing -> list<record> {
    let names = $crds | each {|c| $c.name }
    let nss = kube-items $k namespaces | where {|n| $n.metadata.name not-in $SYSTEM_NS }
    let pods = kube-items $k pods | where {|p| $p.metadata.namespace not-in $SYSTEM_NS }

    let kyverno = if "clusterpolicies.kyverno.io" in $names { (kube-items $k clusterpolicies.kyverno.io) ++ (kube-items $k policies.kyverno.io) } else { [] }
    let kyverno_on = $kyverno | where {|p|
        (($p.spec?.validationFailureAction? | default "" | str lowercase) == "enforce") or ($p.spec?.rules? | default [] | any {|r| ($r.validate?.failureAction? | default "" | str lowercase) == "enforce" })
    }
    let gatekeeper = if "constrainttemplates.templates.gatekeeper.sh" in $names { kube $k get constraints | get -o items | default [] } else { [] }
    let gatekeeper_on = $gatekeeper | where {|c| ($c.spec?.enforcementAction? | default "deny") == "deny" }
    let vap = kube-items $k validatingadmissionpolicybindings
    let vap_on = $vap | where {|b| "Deny" in ($b.spec?.validationActions? | default []) }
    let enforcing = ($kyverno_on | length) + ($gatekeeper_on | length) + ($vap_on | length)
    let audit_only = ($kyverno | length) + ($gatekeeper | length) + ($vap | length) - $enforcing
    let policy_f = if $enforcing > 0 {
        finding clusters $t "admission policy" ok $"($enforcing) enforcing, ($audit_only) audit-only"
    } else if $audit_only > 0 {
        finding clusters $t "admission policy" med $"($audit_only) policies, all audit-only: nothing is blocked"
    } else {
        finding clusters $t "admission policy" high "no Kyverno / Gatekeeper / ValidatingAdmissionPolicy enforcing anything"
    }

    let admins = kube-items $k clusterrolebindings | where {|b| $b.roleRef?.name? == "cluster-admin" }
        | each {|b| $b.subjects? | default [] } | flatten
        | where {|s| not (($s.kind == "Group") and ($s.name in [system:masters kubeadm:cluster-admins])) }
    let public = $admins | where {|s| $s.name in [system:anonymous system:unauthenticated system:authenticated system:serviceaccounts] }
    let sas = $admins | where {|s| ($s.kind == "ServiceAccount") and ($s.namespace? not-in $SYSTEM_NS) }
    let humans = $admins | where {|s| ($s.kind in [User Group]) and ($s.name not-in [system:anonymous system:unauthenticated system:authenticated system:serviceaccounts]) }
    let rbac_f = [
        (if ($public | is-not-empty) { finding clusters $t "cluster-admin scope" crit $"cluster-admin granted to ($public | each {|s| $s.name } | uniq | str join ' ')" })
        (if ($sas | is-not-empty) { finding clusters $t "agent least privilege" high $"service accounts holding cluster-admin: ($sas | each {|s| $'($s.namespace)/($s.name)' } | uniq | short-list)" })
        (if ($humans | is-not-empty) { finding clusters $t "cluster-admin scope" med $"standing cluster-admin: ($humans | each {|s| $'($s.kind):($s.name)' } | uniq | short-list)" })
    ] | compact

    let psa_gap = $nss | where {|n| (label-of $n.metadata "pod-security.kubernetes.io/enforce" | default "privileged") == "privileged" } | each {|n| $n.metadata.name }
    let psa_f = if ($psa_gap | is-empty) {
        finding clusters $t "pod security admission" ok "every namespace enforces baseline/restricted"
    } else { finding clusters $t "pod security admission" med $"($psa_gap | length) namespaces enforce nothing: ($psa_gap | short-list)" }

    let cilium = if "ciliumnetworkpolicies.cilium.io" in $names { kube-items $k ciliumnetworkpolicies.cilium.io } else { [] }
    let segmented = (kube-items $k networkpolicies) ++ $cilium | each {|p| $p.metadata.namespace? } | compact | uniq
    let open_ns = $pods | each {|p| $p.metadata.namespace } | uniq | where {|n| $n not-in $segmented }
    let np_f = if ($open_ns | is-empty) {
        finding clusters $t "network segmentation" ok "every namespace with pods has a NetworkPolicy"
    } else { finding clusters $t "network segmentation" med $"pods run with no NetworkPolicy in: ($open_ns | short-list)" }

    let containers = $pods | each {|p|
        ($p.spec.containers? | default []) ++ ($p.spec.initContainers? | default []) | each {|c| {pod: (ns-name $p), c: $c} }
    } | flatten
    let privileged = $containers | where {|x| $x.c.securityContext?.privileged? == true } | each {|x| $x.pod } | uniq
    let host_ns = $pods | where {|p| ($p.spec.hostNetwork? == true) or ($p.spec.hostPID? == true) or ($p.spec.hostIPC? == true) } | each {|p| ns-name $p }
    let host_path = $pods | where {|p| $p.spec.volumes? | default [] | any {|v| $v.hostPath? != null } } | each {|p| ns-name $p }
    let floating = $containers | where {|x| floating-ref $x.c.image } | each {|x| $x.pod } | uniq
    let pod_f = [
        (if ($privileged | is-not-empty) { finding clusters $t "workload hardening" high $"privileged containers: ($privileged | short-list)" })
        (if ($host_ns | is-not-empty) { finding clusters $t "workload hardening" high $"host network/PID/IPC: ($host_ns | short-list)" })
        (if ($host_path | is-not-empty) { finding clusters $t "workload hardening" med $"hostPath volumes: ($host_path | short-list)" })
        (if ($floating | is-not-empty) { finding clusters $t "pinned images" low $"untagged or :latest images: ($floating | short-list)" })
    ] | compact

    # In-cluster agents: GitOps and infrastructure controllers act with standing credentials.
    let flux_ctrl = kube $k get deployments -A -l app.kubernetes.io/part-of=flux | get -o items | default []
    let flux_f = if ($flux_ctrl | is-empty) { [] } else {
        let default_sa = $flux_ctrl | any {|d| $d.spec.template.spec.containers | any {|c| $c.args? | default [] | any {|a| $a | str starts-with "--default-service-account" } } }
        let objs = [kustomizations.kustomize.toolkit.fluxcd.io helmreleases.helm.toolkit.fluxcd.io] | where {|r| $r in $names } | each {|r| kube-items $k $r } | flatten
        let unscoped = $objs | where {|x| ($x.spec?.serviceAccountName? | is-empty) and ($x.spec?.kubeConfig? == null) }
        let sources = [gitrepositories.source.toolkit.fluxcd.io ocirepositories.source.toolkit.fluxcd.io] | where {|r| $r in $names } | each {|r| kube-items $k $r } | flatten
        let unverified = $sources | where {|s| $s.spec?.verify? == null }
        [
            (finding clusters $t "agent inventory" info $"flux: ($flux_ctrl | each {|d| $d.metadata.name } | str join ' ')")
            (if (not $default_sa) and ($unscoped | is-not-empty) { finding clusters $t "agent least privilege" med $"($unscoped | length) Flux objects apply as the controller's cluster-admin SA: set serviceAccountName or --default-service-account" })
            (if ($unverified | is-not-empty) { finding clusters $t "signed sources" low $"($unverified | length) Flux sources pull without signature verification \(spec.verify\)" })
        ] | compact
    }
    let packages = [providers.pkg.crossplane.io functions.pkg.crossplane.io configurations.pkg.crossplane.io] | where {|r| $r in $names } | each {|r|
        kube-items $k $r | each {|p| {kind: ($r | split row "." | first), name: $p.metadata.name, ref: ($p.spec?.package? | default "")} }
    } | flatten
    let unpinned = $packages | where {|p| floating-ref $p.ref }
    let pcs = $crds | where {|c| $c.name | str starts-with "providerconfigs." } | each {|c|
        kube-items $k $c.name | each {|p| {group: $c.group, name: $p.metadata.name, source: ($p.spec?.credentials?.source? | default "none")} }
    } | flatten
    let static = $pcs | where {|p| ($p.source == "Secret") and not ($p.group =~ '^(kubernetes|helm)\.') }
    let xp_f = if ($packages | is-empty) and ($pcs | is-empty) { [] } else {
        [
            (finding clusters $t "agent inventory" info $"crossplane: ($packages | each {|p| $p.name } | short-list 8)")
            (if ($unpinned | is-not-empty) { finding clusters $t "pinned packages" med $"crossplane packages without a version/digest: ($unpinned | each {|p| $p.ref } | short-list)" })
            (if ($static | is-not-empty) {
                finding clusters $t "agent credentials" high $"static cloud keys in Secrets: ($static | each {|p| $'($p.group)/($p.name)' } | short-list): use InjectedIdentity / Workload Identity"
            } else if ($pcs | is-not-empty) { finding clusters $t "agent credentials" ok "cloud ProviderConfigs use injected identity" })
        ] | compact
    }
    [$policy_f] ++ $rbac_f ++ [$psa_f $np_f] ++ $pod_f ++ $flux_f ++ $xp_f
}

# Reachability, controllers, children and checks for one cluster.
def scan-cluster [k: list<string>, label: string, server: any]: nothing -> record {
    let gke = $label | parse -r '^gke_(?<project>[^_]+)_(?<location>[^_]+)_(?<name>.+)$' | get -o 0
    let gke_f = if ($gke != null) and (has-cmd gcloud) {
        let d = gcloud-json container clusters describe $gke.name --location $gke.location --project $gke.project
        if $d == null {
            [(finding clusters $label "gke control plane" info "gcloud describe failed: no access, or the cluster is gone")]
        } else { gke-findings $label (gke-from-gcloud $d) }
    } else { [] }
    let version = kube $k version | get -o serverVersion.gitVersion
    if $version == null {
        return {label: $label, k: $k, server: $server, version: null, controllers: [], children: [], findings: ([(finding clusters $label "reachable" info "API server unreachable: in-cluster checks skipped")] ++ $gke_f)}
    }
    let crds = crd-index $k
    let names = $crds | each {|c| $c.name }
    let controllers = [
        [crossplane compositions.apiextensions.crossplane.io]
        [capi clusters.cluster.x-k8s.io]
        [flux kustomizations.kustomize.toolkit.fluxcd.io]
        [argocd applications.argoproj.io]
        [kyverno clusterpolicies.kyverno.io]
        [gatekeeper constrainttemplates.templates.gatekeeper.sh]
    ] | where {|p| $p.1 in $names } | each {|p| $p.0 }
    {
        label: $label, k: $k, server: $server, version: $version, controllers: $controllers
        children: (discover-children $k $crds)
        findings: ((cluster-checks $k $label $crds) ++ $gke_f)
    }
}

def match-context [child: record, contexts: list<record>]: nothing -> any {
    if $child.server != null {
        let hit = $contexts | where server == $child.server | get -o 0.name
        if $hit != null { return $hit }
    }
    let n = $child.name
    $contexts | where {|c|
        ($c.name in [$n $"kind-($n)" $"($n)-admin@($n)"]) or ($c.name | str starts-with $"vcluster_($n)_($child.ns)_") or ($c.name | str ends-with $"_($n)")
    } | get -o 0.name
}

# kubeconfig from a child's secret → 0600 file inside the private temp dir.
def secret-kubeconfig [k: list<string>, s: record, dir: string]: nothing -> any {
    let b64 = kube $k get secret -n $s.ns $s.name | get -o data | default {} | transpose key v | where key == $s.key | get -o 0.v
    if ($b64 | is-empty) { return null }
    let path = ^mktemp $"($dir)/kubeconfig.XXXXXX" | str trim
    $b64 | decode base64 | save -f $path
    ^chmod 600 $path
    $path
}

def unaudited [c: record, extra: list<record>, why: string]: nothing -> record {
    $c | insert access "none" | insert context null | insert node null
    | insert findings ($extra ++ [(finding clusters $c.target "child visibility" med $"($c.via) child ($why): unaudited")])
}

def kubeconfig-server [path: string]: nothing -> any {
    try { open --raw $path | from yaml | get -o clusters.0.cluster.server } catch { null }
}

# `seen`: API servers on the path from the root, so a child that points back
# at an ancestor (or the ancestor's own context) is reported, not re-scanned.
def resolve-children [node: record, contexts: list<record>, dir: string, depth: int, seen: list<any>]: nothing -> record {
    let seen = $seen | append $node.server | compact
    let kids = $node.children | each {|c0|
        let c = $c0 | insert target $"($node.label) › ($c0.name)"
        let spec_f = if $c.spec != null { gke-findings $c.target (gke-from-upbound $c.spec) } else { [] }
        let loop = {|access|
            $c | insert access $access | insert context null | insert node null
            | insert findings ($spec_f ++ [(finding clusters $c.target "child visibility" info "resolves to an ancestor cluster: not re-scanned")])
        }
        let ctx = match-context $c $contexts
        if $ctx != null {
            let ctx_server = $contexts | where name == $ctx | get -o 0.server
            if ($ctx == $node.label) or ($ctx_server in $seen) { return (do $loop $"context ($ctx)") }
            return ($c | insert access $"context ($ctx)" | insert context $ctx | insert node null | insert findings $spec_f)
        }
        if $c.secret == null { return (unaudited $c $spec_f "has no kubeconfig secret and no local context") }
        if $depth >= $MAX_DEPTH { return (unaudited $c $spec_f $"is deeper than ($MAX_DEPTH) levels") }
        let path = secret-kubeconfig $node.k $c.secret $dir
        if $path == null { return (unaudited $c $spec_f $"secret ($c.secret.ns)/($c.secret.name) is unreadable") }
        let access = $"secret ($c.secret.ns)/($c.secret.name)"
        let server = kubeconfig-server $path | default $c.server
        if ($server != null) and ($server in $seen) { return (do $loop $access) }
        let sub = scan-cluster ["--kubeconfig" $path] $c.target $server
        if $sub.version == null { return (unaudited $c $spec_f "kubeconfig secret doesn't reach its API server from here") }
        $c | insert access $access | insert context null | insert findings $spec_f
        | insert node (resolve-children $sub $contexts $dir ($depth + 1) $seen)
    }
    $node | update children $kids
}

def node-findings [n: record]: nothing -> list<record> {
    $n.findings ++ ($n.children | each {|c| $c.findings ++ (if $c.node == null { [] } else { node-findings $c.node }) } | flatten)
}

def claimed [n: record]: nothing -> list<string> {
    $n.children | each {|c| [$c.context] ++ (if $c.node == null { [] } else { claimed $c.node }) } | flatten | compact
}

def sev-count [fs: list<record>]: nothing -> record {
    [crit high med low] | each {|s| {k: $s, v: ($fs | where sev == $s | length)} } | transpose -r -d
}

# Flat tree: management cluster, then every child (via, access, posture) indented beneath.
def tree-rows [n: record, nodes: list<record>, indent: string, name: string, via: string, access: string, extra: list<record>, seen: list<string>]: nothing -> list<record> {
    let mgmt = ($n.children | is-not-empty) or ("crossplane" in $n.controllers) or ("capi" in $n.controllers)
    let role = if $mgmt { "management" } else if $indent != "" { "child" } else { "standalone" }
    let row = {
        cluster: $"($indent)($name)"
        role: ([$role] ++ $n.controllers | str join " · ")
        version: ($n.version | default "unreachable")
        via: $via, access: $access
        findings: (sev-count ($n.findings ++ $extra))
    }
    let pad = $indent | str replace -a "├─ " "│  " | str replace -a "└─ " "   "
    let last_i = ($n.children | length) - 1
    let kids = $n.children | enumerate | each {|e|
        let c = $e.item
        let prefix = $pad + (if $e.index == $last_i { "└─ " } else { "├─ " })
        let sub = if $c.context != null { $nodes | where label == $c.context | get -o 0 } else { $c.node }
        if ($sub == null) or ($c.context in $seen) {
            [{cluster: $"($prefix)($c.name)", role: "child", version: "—", via: $c.via, access: $c.access, findings: (sev-count $c.findings)}]
        } else {
            let label = if $c.context != null { $c.context } else { $c.name }
            tree-rows $sub $nodes $prefix $label $c.via $c.access $c.findings ($seen | append $n.label)
        }
    } | flatten
    [$row] ++ $kids
}

def govern-clusters [only: any]: nothing -> record {
    if not (has-cmd kubectl) { return {findings: [(finding clusters kubectl "kubectl installed" info "kubectl not on PATH: skipped")], tree: []} }
    let view = ^kubectl config view -o json | complete
    let view = if $view.exit_code == 0 { $view.stdout | from json } else { {} }
    let servers = $view.clusters? | default [] | each {|c| {cluster: $c.name, server: $c.cluster?.server?} }
    let contexts = $view.contexts? | default [] | each {|c| {name: $c.name, server: ($servers | where cluster == $c.context?.cluster? | get -o 0.server)} }
    let roots = $contexts | where {|c| ($only == null) or ($c.name == $only) }
    if ($roots | is-empty) {
        return {findings: [(finding clusters ($only | default "kubeconfig") "contexts" info "no kube contexts to audit")], tree: []}
    }
    let dir = ^mktemp -d -t toolbelt-govern | str trim
    ^chmod 700 $dir
    let scan = {|cs| $cs | par-each {|c|
        let n = scan-cluster ["--context" $c.name] $c.name $c.server
        resolve-children $n $contexts $dir 1 []
    } }
    mut nodes = do $scan $roots
    # --context: children reached through other local contexts still get scanned.
    loop {
        let have = $nodes | each {|n| $n.label }
        let want = $nodes | each {|n| claimed $n } | flatten | uniq | where {|c| $c not-in $have }
        if ($want | is-empty) { break }
        $nodes = $nodes ++ (do $scan ($contexts | where name in $want))
    }
    rm -rf $dir
    let nodes = $nodes
    # Contexts that are someone's child render under their parent, not as roots.
    let taken = $nodes | each {|n| claimed $n } | flatten | uniq
    let top = $nodes | where {|n| $n.label not-in $taken }
    let mgmt = $nodes | where {|n| ($n.children | is-not-empty) or ("crossplane" in $n.controllers) or ("capi" in $n.controllers) }
    let mgmt_f = if ($mgmt | is-empty) {
        [(finding clusters kubeconfig "management cluster" info $"none among ($roots | length) contexts: no Crossplane/CAPI and no discovered children")]
    } else {
        $mgmt | each {|n| finding clusters $n.label "management cluster" info $"($n.children | length) children: ($n.children | each {|c| $"($c.name) [($c.via)]" } | short-list)" }
    }
    {
        findings: (($nodes | each {|n| node-findings $n } | flatten) ++ $mgmt_f)
        tree: ($top | each {|n| tree-rows $n $nodes "" $n.label "" "context" [] [] } | flatten)
    }
}

# ── governance: report ──

def sev-color [s: string]: nothing -> string {
    match $s { "crit" => "red_reverse", "high" => "red_bold", "med" => "yellow", "low" => "cyan", "ok" => "green", _ => "dark_gray" }
}

def counts-str [c: record]: nothing -> string {
    let parts = [crit high med low] | where {|s| ($c | get $s) > 0 } | each {|s| $"(ansi (sev-color $s))($s)(ansi reset) ($c | get $s)" }
    if ($parts | is-empty) { $"(ansi green)clean(ansi reset)" } else { $parts | str join "  " }
}

# Security & governance audit (read-only): shells, GCP, MCP servers, agents, and
# every kube cluster: management clusters and the children they created.
def "main govern" [
    --scope (-s): string     # comma-separated: shells,gcp,mcp,agents,clusters (default: all)
    --context (-c): string   # clusters: start from this kube context only (children still followed)
    --all (-a)               # also list info and passing checks
    --json                   # {findings, clusters} as JSON
] {
    let repo = repo-dir
    let scopes = if $scope == null { $GOVERN_SCOPES } else { $scope | split row "," | each {|s| $s | str trim } }
    let bad = $scopes | where {|s| $s not-in $GOVERN_SCOPES }
    if ($bad | is-not-empty) { error make {msg: $"unknown scope: ($bad | str join ', ') \(known: ($GOVERN_SCOPES | str join ', ')\)"} }
    let inv = if ($scopes | any {|s| $s in [shells agents] }) { invocations } else { [] }
    let parts = $scopes | par-each --keep-order {|s|
        match $s {
            "shells" => {findings: (govern-shells $repo $inv), tree: []}
            "gcp" => {findings: (govern-gcp $repo), tree: []}
            "mcp" => {findings: (govern-mcp $repo), tree: []}
            "agents" => {findings: (govern-agents $repo $inv), tree: []}
            "clusters" => (govern-clusters $context)
        }
    }
    let findings = $parts | each {|p| $p.findings } | flatten
        | insert o {|f| $SEVERITIES | enumerate | where item == $f.sev | get 0.index }
        | sort-by o scope target | reject o
    let tree = $parts | each {|p| $p.tree } | flatten
    if $json { return ({findings: $findings, clusters: $tree} | to json) }

    print $"(ansi white_bold)toolbelt govern(ansi reset) (ansi dark_gray)· read-only · (date now | format date '%Y-%m-%d %H:%M')(ansi reset)"
    header "POSTURE"
    print ($scopes | each {|s|
        let fs = $findings | where scope == $s
        let c = sev-count $fs
        let worst = $fs | where sev in [crit high med low] | get -o 0
        {
            scope: $s
            posture: (counts-str $c)
            passing: ($fs | where sev == ok | length)
            worst: (if $worst == null { "" } else { $"($worst.target): ($worst.check)" })
        }
    } | table -i false)
    if ($tree | is-not-empty) {
        header "CLUSTERS — management → children"
        print ($tree | update findings {|r| counts-str $r.findings } | update role {|r|
            $r.role | str replace -r '^management' $"(ansi yellow_bold)management(ansi reset)"
        } | table -i false)
    }
    let shown = if $all { $findings } else { $findings | where sev in [crit high med low] }
    header $"FINDINGS(if $all { '' } else { ' — crit → low; --all adds info and passing checks' })"
    if ($shown | is-empty) { print $"(ansi green)nothing to fix(ansi reset)" }
    for f in $shown {
        print $" (ansi (sev-color $f.sev))($f.sev | fill -w 4)(ansi reset) (ansi dark_gray)($f.scope | fill -w 8)(ansi reset) ($f.target) (ansi dark_gray)· ($f.check)(ansi reset)"
        if $f.detail != "" { print $"      (ansi dark_gray)↳(ansi reset) ($f.detail)" }
    }
}
