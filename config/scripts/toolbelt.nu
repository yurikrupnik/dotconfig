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
#   nu-command   nu built-ins / keywords / plugin commands run in nu history
#   nu-module    $env.NU_LIB_DIRS modules, init modules config.nu loads
#                (starship/zoxide/mise/direnv caches), defs in config/env/generated.nu
#   nu-plugin    registered plugins (`plugin list`)
#   The nu rows read the user's nu scope from one `nu --config … --env-config …`
#   spawn. A nu history head resolves as nu does: alias → custom → built-in →
#   external, with nu's own scope aliases (generated.nu skips config.toml
#   aliases that would shadow a built-in, so nu `ls` is the built-in, not eza);
#   a built-in head is the nu-command's and no external bin's.
#
# Usage evidence, per shell
#   zsh   .zsh_history holds DISTINCT lines only (hist_ignore_all_dups), so the
#         run log written by the preexec/precmd hooks in zsh/.config/zsh/.zshrc
#         counts real runs, with exit status and duration (older lines have
#         neither). zsh uses = max(distinct history lines, logged runs).
#   nu    history.sqlite3 (every run, with exit status and duration) + legacy history.txt
#   bash  ~/.bash_history
#   Custom code invoked by other custom code is credited transitively:
#   `u` → update → just regen → shells.nu.
#
# nu (`toolbelt nu`, read-only)
#   nu binaries on PATH, config wiring, NU_LIB_DIRS, plugins, history stats;
#   nu-native commands run in nu history; modules; every .nu script
#   (config/scripts, scripts/**, any `nu x.nu` / `./x.nu` in history, paths
#   resolved via the sqlite `cwd`) with runs, failures and time over --since.
#
# Value model
#   third-party  you wrote 0 lines; every use is a task done without code.
#                missing | unused | rare (<3 uses) | active (<20) | core
#                gui / editor / lib: not launched from a shell, usage n/a
#                (nu-command / nu-plugin too: nu's own code, not yours)
#   custom       cost = lines you maintain (alias = 1, function = commands).
#                saved = keystrokes saved per use vs typing the expansion.
#                dead (0 uses) | marginal (<1 use per 10 lines) | pays off
#                (nu-module too; init: a generated init module, usage n/a)
#   gaps         command prefixes typed by hand ≥5 times with no wrapper:
#                where writing an alias/function would pay.
#
# Cost (`toolbelt cost`, read-only)
#   Per command over a window (--since): runs, failures, total and average time,
#   last run, from the zsh run log and nu history.sqlite3. A line counts for
#   every command it runs (pipeline segments, alias targets), as in the
#   dashboard. Runs logged without exit status/duration count as runs; their
#   failures and time are unknown (null), not 0. With calltrace on PATH,
#   `calltrace stats` adds every process its traced runs spawned, and the
#   caller → callee edges between them.
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
#   With claude it prints the call's tokens and API-list cost (`usage` in --json) and tags
#   the Claude Code OTel exports caller=toolbelt-manage (dashboard: shluviza apps/ai-usage).

const RARE = 3
const CORE = 20
const PAYOFF = 0.1
const GAP_MIN = 5
const THIRD_PARTY = [brew cask mise cargo node uv local]
const SOURCE_ORDER = [brew cask mise cargo node uv local alias function script repo-script just nu-command nu-module nu-plugin]
# Not code you wrote: scored with the third-party tiers. nu-command/nu-plugin
# own no PATH bins, so they stay out of THIRD_PARTY (owners, shadowing).
const OFF_THE_SHELF = $THIRD_PARTY ++ [nu-command nu-plugin]
const NU_SOURCES = [nu-command nu-module nu-plugin]
const WRAPPERS = [sudo time nohup exec command builtin noglob env caffeinate sfw]
const RUNNERS = [nu bash sh zsh]
const JUST_VALUE_FLAGS = [-f --justfile -d --working-directory --set --shell --dotenv-path]
const GAP_IGNORE = [cd z exit clear echo man which history source export open]
const EDITOR_BIN = '(language-server|langserver|^pyright|^rust-analyzer$)'
# invocation source → shell: zsh history / run log, nu sqlite / txt, bash history
const SHELL_OF = {zh: zsh, zl: zsh, nd: nu, nt: nu, bh: bash}

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

# `local`: installed from a path or git checkout (key "name ver (path+file://…)").
# Those are dev builds of the user's own crates; liner.toml can only name
# crates.io packages, where the same name may be someone else's crate.
def cargo-crates []: nothing -> list<record> {
    let p = $env.CARGO_HOME? | default ($env.HOME | path join .cargo) | path join .crates2.json
    if not ($p | path exists) { return [] }
    open $p | get installs | transpose key v
    | each {|r| {name: ($r.key | split row " " | first), bins: $r.v.bins, local: ($r.key !~ '\(registry\+')} }
}

def cargo-items [repo: string, crates: list<record>]: nothing -> list<record> {
    # cargo-liner installs itself (justfile cargo-install); it is never listed in liner.toml.
    let declared = try { open ($repo | path join config cargo liner.toml) | get packages | columns } catch { [] } | append cargo-liner
    $declared | append $crates.name | uniq | each {|n|
        let c = $crates | where name == $n | get -o 0
        mk {
            source: cargo name: $n bins: ($c.bins? | default [$n]) installed: ($c != null)
            managed: (if $n in $declared { "declared" } else if ($c.local? | default false) { "local" } else { "drift" })
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

# A [functions.*] table's command list: `commands` (list) or `command` (one string),
# the two forms shells.nu accepts.
def fn-commands [f: record]: nothing -> list<string> {
    if "commands" in $f { $f.commands } else { [($f.command? | default "")] | compact --empty }
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
        let cmds = fn-commands $f.f
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

# zl and nd rows also carry exit and ms (duration); null = not recorded
# (run-log lines from before the precmd hook, old nu rows), never 0.
def invocations []: nothing -> list<record> {
    let zh = zsh-history-paths | each {|p| read-zsh-history $p } | flatten
    let zl = if (zsh-log-path | path exists) {
        open --raw (zsh-log-path) | decode utf-8 | lines
        # <start>\t<exit>\t<ms>\t<command>, or legacy <start>\t<command>
        | parse -r '^(?<ts>\d+)\t(?:(?<exit>-?\d+)\t(?<ms>-?\d+)\t)?(?<line>.*)$'
        | each {|r| {
            src: zl, ts: (epoch ($r.ts | into int) s), line: $r.line
            exit: (if $r.exit == null { null } else { $r.exit | into int })
            ms: (if $r.ms == null { null } else { $r.ms | into int })
        } }
    } else { [] }
    let nd = if (nu-db-path | path exists) {
        open (nu-db-path) | query db "SELECT command_line AS line, start_timestamp AS ts, exit_status AS exit, duration_ms AS ms, cwd FROM history"
        | each {|r| {src: nd, ts: (if $r.ts == null { null } else { epoch $r.ts ms }), line: $r.line, exit: $r.exit, ms: $r.ms, cwd: $r.cwd} }
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
# native: the plain words again, unless a wrapper ran them (`sudo ls` is
#         external): what nu would resolve as a command name.
# script: the .nu/.sh path as typed (`nu x.nu`, `./x.nu`), else null.
def seg-parse [seg: string, aliases: record]: nothing -> record {
    mut words = $seg | str trim | str trim -l -c "(" | str trim -l -c "{" | split row -r '\s+' | where $it != ""
    mut tokens = []
    mut wrapped = false
    while ($words | is-not-empty) and (($words.0 =~ '^[A-Za-z_][A-Za-z0-9_]*=') or ($words.0 in $WRAPPERS)) {
        if $words.0 == "sfw" { $tokens = $tokens | append "sfw" }
        if $words.0 in $WRAPPERS { $wrapped = true }
        $words = $words | skip 1
    }
    if ($words | is-empty) { return {tokens: $tokens, words: [], native: [], script: null} }
    let head = $words.0 | str trim -l -c "^"
    let args = $words | skip 1
    mut script = if ($head =~ '\.(nu|sh)$') { $head } else { null }
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
        let s = $args | where {|a| not ($a | str starts-with "-") } | get -o 0 | default "" | str trim -c "'" | str trim -c '"'
        if ($s =~ '\.(nu|sh)$') { $tokens = $tokens | append ($s | path basename); $script = $s }
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
    {tokens: $tokens, words: $plain, native: (if $wrapped { [] } else { $plain }), script: $script}
}

def line-parse [line: string, aliases: record]: nothing -> record {
    let segs = $line | split row -r '\|\||&&|;|\||\$\(|`' | each {|s| seg-parse $s $aliases }
    {
        tokens: ($segs | get tokens | flatten | uniq)
        heads: ($segs | get native | where {|w| $w | is-not-empty })
        scripts: ($segs | get script | compact)
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
    if $it.source == "nu-module" and $it.kind == "init" { return "init" }
    if $it.source in $OFF_THE_SHELF {
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
    let jobs = [inv hist nu] | par-each --keep-order {|j|
        match $j { "inv" => (inventory $repo), "hist" => (invocations), _ => (nu-scope) }
    }
    let refs = config-refs $repo
    let items = $jobs.0 | each {|it|
        if not ($it.source in $THIRD_PARTY and $it.kind == "shell") { return $it }
        if ($it.bins | any {|b| $b in $refs.rc }) { return ($it | update kind init) }
        if ($it.bins | any {|b| $b in $refs.editor }) { return ($it | update kind editor) }
        $it
    }
    let inv = $jobs.1
    let scope = $jobs.2
    let native = nu-native $scope
    let aliases = $items | where source == alias | select name desc | transpose -r -d
    let parsed = parse-history $inv $aliases $scope $native
    let counts = aggregate ($parsed | each {|p| $p.tokens | each {|t| {src: $p.src, ts: $p.ts, key: $t} } } | flatten)
    let items = $items ++ (nu-items $repo $scope $native $counts)
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
        let s = if $it.source in $NU_SOURCES { nu-stats $mine $counts } else {
            sum-stats ($mine | each {|b| $counts | get -o $b } | compact)
        }
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
    $tools | where source not-in $OFF_THE_SHELF | each {|t| {
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
        # nu's own commands and modules aren't tools on PATH: the card stays about those.
        let used = $data.tools | where {|t| $t.source not-in $NU_SOURCES and ($t | get $col) > 0 }
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

    let third = $t | where {|r| $r.source in $OFF_THE_SHELF and $r.installed }
    let shell3 = $third | where kind == shell
    let custom = $t | where source not-in $OFF_THE_SHELF
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
    print $"(ansi dark_gray)drill down: toolbelt tools | shells | nu | value | gaps | cost | ui | govern | manage   \(--json for data\)(ansi reset)"
}

# Every inventoried tool with status and per-shell usage.
def "main tools" [
    --source (-s): string   # brew|cask|mise|cargo|node|uv|local|alias|function|script|repo-script|just|nu-command|nu-module|nu-plugin
    --status: string        # core|active|rare|unused|missing|gui|editor|lib|init|pays off|marginal|dead
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

# ── cost ─────────────────────────────────────────────────────────────────────
# `toolbelt cost`: where interactive time goes. Only sources that log every run
# with its start time count: the zsh run log and nu's sqlite history.

const COST_SOURCES = [zl nd]

# rows {ts exit ms} → runs; failures and time only over `timed` runs (exit and
# ms recorded), null when none were — unknown is not 0.
def cost-of [rows: list<record>]: nothing -> record {
    let timed = $rows | where {|r| $r.exit != null and $r.ms != null }
    let n = $timed | length
    let total = if $n == 0 { null } else { $timed | get ms | math sum }
    {
        runs: ($rows | length)
        timed: $n
        failures: (if $n == 0 { null } else { $timed | where {|r| $r.exit != 0 } | length })
        total_ms: $total
        avg_ms: (if $n == 0 { null } else { $total / $n | math round })
        last: (if ($rows | is-empty) { null } else { $rows | get ts | sort | last })
    }
}

# Keyed like the dashboard: a line counts for every command it runs.
def cost-rows [runs: list<record>, aliases: record]: nothing -> list<record> {
    let keyed = $runs | par-each {|r|
        line-parse $r.line $aliases | get tokens | each {|k| {key: $k, ts: $r.ts, exit: $r.exit, ms: $r.ms} }
    } | flatten
    if ($keyed | is-empty) { return [] }
    $keyed | group-by key | transpose command rows | each {|g| {command: $g.command} | merge (cost-of $g.rows) }
}

# Compact, so the tables fit a terminal: 445ms · 12.3s · 26m52s · 3h05m.
def dur [ms: int]: nothing -> string {
    if $ms < 1000 { return $"($ms)ms" }
    if $ms < 60_000 { return $"($ms / 1000 | math round -p 1)s" }
    let s = $ms // 1000
    if $s < 3600 { return $"($s // 60)m($s mod 60 | fill -a r -c 0 -w 2)s" }
    $"($s // 3600)h($s mod 3600 // 60 | fill -a r -c 0 -w 2)m"
}

def cost-view []: list<record> -> list<record> {
    each {|r| $r
        | update failures ($r.failures | default "?")
        | update total_ms (if $r.total_ms == null { "?" } else { dur $r.total_ms })
        | update avg_ms (if $r.avg_ms == null { "?" } else { dur $r.avg_ms })
        | update last (fmt-date $r.last)
        | rename -c {total_ms: total, avg_ms: avg}
    }
}

# `calltrace stats --json` over the same window: {stats, note}; stats is null
# (note says why) when calltrace is absent, fails or prints something else.
def calltrace-stats [since: duration, top: int]: nothing -> record {
    if not (has-cmd calltrace) {
        return {stats: null, note: "calltrace not on PATH — install from toolkit: bun nx run calltrace:install"}
    }
    let secs = [($since / 1sec | math floor) 0] | math max
    let r = ^calltrace stats --json --top $top --since $"($secs)s" | complete
    if $r.exit_code != 0 {
        let why = $r.stderr | str trim | lines | get -o 0 | default $"exit ($r.exit_code)"
        return {stats: null, note: $"calltrace stats failed: ($why)"}
    }
    let j = try { $r.stdout | from json } catch { null }
    if ($j | describe) !~ '^record' { return {stats: null, note: "calltrace stats printed no JSON record"} }
    {stats: $j, note: ""}
}

def traced-view [cmds: list<record>]: nothing -> list<record> {
    $cmds | each {|c|
        let n = $c.count? | default 0
        let wall_ms = ($c.wall_us? | default 0) / 1000 | math round
        {
            command: ($c.label? | default "?")
            procs: $n
            failures: ($c.failures? | default 0)
            total: (dur $wall_ms)
            avg: (if $n == 0 { "?" } else { dur ($wall_ms / $n | math round) })
            cpu: (dur (($c.cpu_us? | default 0) / 1000 | math round))
            max_rss: (($c.max_rss_kb? | default 0) * 1KiB)
        }
    }
}

# Where interactive time goes, per command: runs, failures, total and average
# time, last run (zsh run log + nu history); with calltrace installed, also the
# processes its traced runs spawned and the caller → callee edges between them.
def "main cost" [
    --since: duration = 30day   # only runs started within this window
    --top (-n): int = 25        # rows per table
    --shell: string = "all"     # zsh|nu|all
    --json                      # {since, shells, commands, traced} as JSON
] {
    let known = $COST_SOURCES | each {|s| $SHELL_OF | get $s }
    if $shell != "all" and $shell not-in $known {
        error make {msg: $"unknown shell: ($shell) \(known: ($known | append all | str join ', ')\)"}
    }
    let srcs = $COST_SOURCES | where {|s| $shell == "all" or ($SHELL_OF | get $s) == $shell }
    let cutoff = (date now) - $since
    let runs = invocations | where {|i| $i.src in $srcs and $i.ts != null and $i.ts >= $cutoff }
    let aliases = try { open (repo-dir | path join config shell config.toml) | get -o aliases | default {} } catch { {} }
    let shells = $srcs | each {|s| {shell: ($SHELL_OF | get $s)} | merge (cost-of ($runs | where src == $s)) }
    let commands = cost-rows $runs $aliases
        | insert k {|r| $r.total_ms | default (-1) } | sort-by -r k runs | reject k
        | first $top
    let traced = calltrace-stats $since $top
    if $json { return ({since: $cutoff, shells: $shells, commands: $commands, traced: $traced.stats} | to json) }

    print $"(ansi white_bold)toolbelt cost(ansi reset) (ansi dark_gray)· ($shells | get shell | str join ' + ') since (fmt-date $cutoff) · (date now | format date '%Y-%m-%d %H:%M')(ansi reset)"
    header "SHELLS"
    print ($shells | cost-view | table -i false)
    header $"COMMANDS — top ($top) by total time; a line counts for every command it runs"
    print ($commands | cost-view | table -i false)
    if ($shells | any {|s| $s.timed < $s.runs }) {
        print $"(ansi dark_gray)timed = runs with a recorded exit status and duration \(older history has none\); failures/total/avg cover only those, ? = none did(ansi reset)"
    }
    header "TRACED (calltrace) — every process its traced runs spawned"
    if $traced.stats == null {
        print $"(ansi dark_gray)($traced.note)(ansi reset)"
    } else {
        let st = $traced.stats
        print $"(ansi dark_gray)($st.runs? | default 0) runs · ($st.procs? | default 0) processes(ansi reset)"
        print (traced-view ($st.commands? | default []) | table -i false)
        header "CALL EDGES — caller → callee"
        print ($st.edges? | default [] | first $top | each {|e| {caller: $e.caller?, callee: $e.callee?, count: $e.count?} } | table -i false)
    }
}

# ── nu ───────────────────────────────────────────────────────────────────────
# `toolbelt nu` and the nu-* inventory rows. What nu resolves (commands,
# aliases, plugins, NU_LIB_DIRS, history config) comes from one spawn of the
# user's nu with its config (nu-scope); modules, scripts and history are files.

const NU_CONFIG_FILES = [config.nu env.nu login.nu generated.nu]
const NU_SCOPE = r#'{
    commands: (scope commands | where type in [built-in keyword plugin custom] | select name type category description)
    aliases: (scope aliases | select name expansion)
    modules: (scope modules | select name file)
    plugins: (plugin list | each {|p| {
        name: $p.name
        version: ($p.version? | default "")
        status: ($p.status? | default "")
        filename: ($p.filename? | default "")
        commands: ($p.commands? | default [] | each {|c| if ($c | describe) =~ '^record' { $c.name } else { $c } })
    } })
    lib_dirs: ($env.NU_LIB_DIRS? | default [] | append $NU_LIB_DIRS | uniq)
    plugin_path: $nu.plugin-path
    history: ($env.config.history? | default {})
} | to json -r'#

def nu-config-dir []: nothing -> string { $env.HOME | path join .config nushell }

# The user's nu as an interactive shell sees it: the nu on PATH with
# config.nu/env.nu, run from the temp dir so no project hook fires. {} when it
# fails: nu heads then stay unresolved (external), never guessed.
def nu-scope []: nothing -> record {
    if not (has-cmd nu) { return {} }
    let dir = nu-config-dir
    let flags = [[--config config.nu] [--env-config env.nu]]
        | where {|f| $dir | path join $f.1 | path exists }
        | each {|f| [$f.0 ($dir | path join $f.1)] } | flatten
    let r = do { cd $nu.temp-dir; ^nu ...$flags -c $NU_SCOPE | complete }
    if $r.exit_code != 0 { return {} }
    try { $r.stdout | from json } catch { {} }
}

# command name → its scope row (type, category, description).
def nu-native [scope: record]: nothing -> record {
    let cmds = $scope.commands? | default []
    if ($cmds | is-empty) { return {} }
    $cmds | each {|c| {k: $c.name, v: $c} } | transpose -r -d
}

# Commands nu runs for one line's heads: the longest scope match of the first
# 3/2/1 plain words (`str replace`, `sys cpu`). Alias heads are the caller's.
def nu-heads [heads: list, native: record, aliases: list<string>]: nothing -> list<string> {
    $heads | where {|w| $w.0 not-in $aliases } | each {|w|
        [3 2 1] | each {|n| $w | first $n | str join " " } | where {|c| $c in $native } | get -o 0
    } | compact | uniq
}

# nu history rows (nd, nt): a native head (custom or built-in, which win over
# PATH) is keyed `nu:<command>` and no longer credited to an external bin.
def nu-retoken [native: record, aliases: list<string>]: record -> record {
    let p = $in
    if $p.src not-in [nd nt] { return $p }
    let cmds = nu-heads $p.heads $native $aliases
    if ($cmds | is-empty) { return $p }
    let firsts = $cmds | each {|c| $c | split row " " | first }
    $p | update tokens ($p.tokens | where {|t| $t not-in $firsts } | append ($cmds | each {|c| $"nu:($c)" }))
}

# History rows → line-parse + nu resolution. nu rows (nd, nt) expand nu's scope
# aliases, zsh/bash rows config.toml's; if the scope spawn failed, nu rows fall
# back to config.toml's.
def parse-history [inv: list<record>, aliases: record, scope: record, native: record]: nothing -> list<record> {
    let nu_aliases = if ($scope | is-empty) { $aliases } else {
        let a = $scope.aliases? | default []
        if ($a | is-empty) { {} } else { $a | select name expansion | transpose -r -d }
    }
    let nu_names = $nu_aliases | columns
    $inv | par-each {|i|
        if $i.src in [nd nt] {
            $i | merge (line-parse $i.line $nu_aliases) | nu-retoken $native $nu_names
        } else { $i | merge (line-parse $i.line $aliases) }
    }
}

# dotconfig (under the repo), generated (dotconfig output/, ~/.cache init
# scripts) or external (installed by something else, e.g. ~/.local/lib/devkit).
def nu-owner [p: string, repo: string]: nothing -> string {
    let real = $p | path expand
    let repo = $repo | path expand
    let cache = $env.XDG_CACHE_HOME? | default ($env.HOME | path join .cache) | path expand
    if ($real | str starts-with ($repo | path join output)) or ($real | str starts-with $cache) { return "generated" }
    if ($real | str starts-with $repo) { "dotconfig" } else { "external" }
}

# What each module adds, as nu names it (`devkit devkit up`, `init __zoxide_z`):
# one `nu -n` spawn loads every module in its own block and diffs scope
# commands/aliases against the bare scope. A module that breaks the batch is
# retried alone; one that fails alone adds [] (path → names).
def nu-module-commands [loads: list<record>]: nothing -> record {
    if ($loads | is-empty) or not (has-cmd nu) { return {} }
    let run = {|ls|
        let entries = $ls | each {|l|
            let p = $l.path | to nuon
            $p + ': (do { ' + $l.verb + ' ' + $p + '; (scope commands | get name) ++ (scope aliases | get name) | where {|n| $n not-in $base } })'
        }
        let src = 'let base = (scope commands | get name) ++ (scope aliases | get name); {' + ($entries | str join ', ') + '} | to json -r'
        let r = do { cd $nu.temp-dir; ^nu -n -c $src | complete }
        if $r.exit_code == 0 { try { $r.stdout | from json } catch { null } } else { null }
    }
    let all = do $run $loads
    if $all != null { return $all }
    $loads | par-each {|l| do $run [$l] | default {($l.path): []} } | reduce -f {} {|r, acc| $acc | merge $r }
}

# Every nu module: NU_LIB_DIRS entries (a dir with mod.nu, or x.nu), the init
# scripts config.nu/env.nu `use`/`source` by path (starship, zoxide, mise, direnv
# caches: kind init) and the defs config/env/login/generated.nu add to the scope
# (one row per def). A load only counts (loaded_by) when the module reached the
# user's scope: a `use` inside an `if` block is scoped to that block, so it is
# listed under ineffective_loads. bins = exports: runs are nu heads resolved to them.
def nu-modules [repo: string, scope: record]: nothing -> list<record> {
    let dir = nu-config-dir
    let cfgs = $NU_CONFIG_FILES | each {|f| {name: $f, path: ($dir | path join $f)} } | where {|c| $c.path | path exists }
        | insert text {|c| open --raw $c.path | lines | where {|l| not ($l | str trim | str starts-with "#") } | str join "\n" }
    let loaders = {|name| $cfgs | where {|c| $c.text =~ ('(?m)^\s*(?:export\s+)?(?:overlay\s+)?(?:use|source)\s+\S*\b' + $name + '\b') } | get name }
    let lib = $scope.lib_dirs? | default [] | where {|d| $d | path exists } | each {|d|
        ls $d | get name | where {|p| ($p | str ends-with ".nu") or ($p | path join mod.nu | path exists) } | each {|p|
            let is_dir = $p | path join mod.nu | path exists
            let name = $p | path basename | str replace -r '\.nu$' ''
            let entry = (if $is_dir { $p | path join mod.nu } else { $p }) | path expand
            let files = if $is_dir { glob $"($p | path expand)/**/*.nu" } else { [$entry] }
            {
                name: $name path: $p origin: "lib-dir" managed: (nu-owner $p $repo) verb: "use" entry: $entry
                loc: ($files | each {|f| loc $f } | append 0 | math sum) by: (do $loaders $name) kind: "shell"
            }
        }
    } | flatten
    let init = $cfgs | where name in [config.nu env.nu] | each {|c|
        $c.text | parse -r '(?m)^\s*(?<verb>use|source)\s+(?<p>[~/]\S*\.nu)\b'
        | each {|m| {by: $c.name, verb: $m.verb, path: ($m.p | path expand)} }
    } | flatten | where {|r| $r.path | path exists } | group-by path | transpose path rs | each {|g|
        let stem = $g.path | path basename | str replace -r '\.nu$' ''
        {
            name: (if $stem in [init mod] { $g.path | path dirname | path basename } else { $stem })
            path: $g.path origin: "init" managed: (nu-owner $g.path $repo) verb: $g.rs.0.verb entry: $g.path
            loc: (loc $g.path) by: ($g.rs.by | uniq) kind: "init"
        }
    }
    let added = nu-module-commands ($lib ++ $init | select path verb)
    let in_scope = ($scope.commands? | default [] | get name) ++ ($scope.aliases? | default [] | get name)
    let scope_files = $scope.modules? | default [] | get file | compact | each {|f| $f | path expand }
    let mods = $lib ++ $init | each {|m|
        let exports = $added | get -o $m.path | default []
        let live = ($exports | any {|e| $e in $in_scope }) or ($m.entry in $scope_files)
        {
            name: $m.name path: $m.path origin: $m.origin managed: $m.managed exports: $exports loc: $m.loc
            loaded_by: (if $live { $m.by } else { [] }) ineffective_loads: (if $live { [] } else { $m.by })
            kind: $m.kind bins: $exports
        }
    }
    let customs = $scope.commands? | default [] | where type == custom | get name
    let defs = $cfgs | each {|c|
        let lines = open --raw $c.path | lines
        let by = if $c.name in [config.nu env.nu login.nu] { [$c.name] } else { do $loaders ($c.name | str replace -r '\.nu$' '') }
        $lines | enumerate | each {|l|
            let m = $l.item | parse -r r#'^(?<ind>\s*)(?:export\s+)?def\s+(?:--?[\w-]+\s+)*(?:"(?<q>[^"]+)"|'(?<s>[^']+)'|(?<n>[^\s\[]+))'#
            if ($m | is-empty) { null } else { {i: $l.index, ind: $m.0.ind, name: ([$m.0.q $m.0.s $m.0.n] | compact --empty | first)} }
        } | compact | where name in $customs | each {|d|
            # body: the def line up to its closing `}` at the def's indent
            let body = if ($lines | get $d.i | str trim | str ends-with "}") { [($lines | get $d.i)] } else {
                $lines | skip $d.i | take until {|x| $x == $"($d.ind)}" } | append "}"
            }
            {
                name: $d.name path: $c.path origin: "config" managed: (nu-owner $c.path $repo) exports: [$d.name]
                loc: ($body | where {|x| let t = $x | str trim; $t != "" and not ($t | str starts-with "#") } | length)
                loaded_by: $by ineffective_loads: [] kind: "shell" bins: [$d.name]
            }
        }
    } | flatten
    $mods ++ $defs
}

# Runs of nu-native names: nu history heads nu resolved to them (`nu:<name>`).
def nu-stats [names: list<string>, counts: record]: nothing -> record {
    sum-stats ($names | each {|b| $counts | get -o $"nu:($b)" } | compact)
}

# nu-command rows exist only for commands run (counts' `nu:<command>` keys,
# from nu-retoken): never nu's ~500 idle built-ins.
def nu-items [repo: string, scope: record, native: record, counts: record]: nothing -> list<record> {
    let plugins = $scope.plugins? | default []
    let cmds = $counts | columns | where {|k| $k starts-with "nu:" }
        | each {|k| $native | get -o ($k | str substring 3..) } | compact | where type != custom
        | each {|c| mk {
            source: nu-command name: $c.name bins: [$c.name] desc: ($c.description? | default "")
            managed: (if $c.type == plugin { $plugins | where {|p| $c.name in $p.commands } | get -o 0.name | default plugin } else { "nu" })
        } }
    let mods = nu-modules $repo $scope | each {|m| mk {
        source: nu-module name: $m.name bins: $m.bins managed: $m.managed kind: $m.kind
        desc: (tilde $m.path) code: (if $m.managed == "generated" { 0 } else { $m.loc })
    } }
    let plugs = $plugins | each {|p| mk {
        source: nu-plugin name: $p.name bins: $p.commands installed: ($p.filename | path exists)
        managed: "registered" desc: $"($p.version) (tilde $p.filename)"
    } }
    $cmds ++ $mods ++ $plugs | insert id {|it| $"($it.source):($it.name)" }
}

# The manager a nu binary comes from, by the root it (or its target) lives under
# (brew: HOMEBREW_PREFIX, which `brew shellenv` sets on macOS and Linux).
def nu-bin-source [p: string]: nothing -> any {
    let real = $p | path expand
    [
        [mise ($env.MISE_DATA_DIR? | default ($env.HOME | path join .local share mise))]
        [brew ($env.HOMEBREW_PREFIX? | default "")]
        [cargo ($env.CARGO_HOME? | default ($env.HOME | path join .cargo))]
        [node ($env.BUN_INSTALL? | default ($env.HOME | path join .bun))]
        [uv (uv-tools-dir)]
        [local ($env.HOME | path join .local bin)]
    ] | where {|r| $r.1 != "" and (($p | str starts-with $r.1) or ($real | str starts-with $r.1)) } | get -o 0.0
}

def in-window [ts: any, cutoff: any]: nothing -> bool {
    $ts != null and ($cutoff == null or $ts >= $cutoff)
}

# cost-of over a row set's timed sources (zsh run log, nu sqlite) in the window.
def window-cost [rows: list<record>, cutoff: any]: nothing -> record {
    cost-of ($rows | where {|r| $r.src in $COST_SOURCES and (in-window $r.ts $cutoff) } | each {|r| {ts: $r.ts, exit: $r.exit?, ms: $r.ms?} })
}

def nu-history-stats [scope: record]: nothing -> record {
    let db = nu-db-path
    let txt = nu-txt-path
    let h = $scope.history? | default {}
    let size = {|p| if ($p | path exists) { ls $p | get 0.size | into int } else { 0 } }
    let s = if ($db | path exists) {
        open $db | query db "SELECT COUNT(*) AS rows, MIN(start_timestamp) AS first, MAX(start_timestamp) AS last, SUM(exit_status IS NOT NULL AND duration_ms IS NOT NULL) AS timed, SUM(exit_status IS NOT NULL AND duration_ms IS NOT NULL AND exit_status != 0) AS failures FROM history" | first
    } else { {rows: 0, first: null, last: null, timed: null, failures: null} }
    {
        format: ($h.file_format? | default null)
        max_size: ($h.max_size? | default null)
        sqlite: {
            path: $db exists: ($db | path exists) rows: $s.rows bytes: (do $size $db) wal_bytes: (do $size $"($db)-wal")
            first: (if $s.first == null { null } else { epoch $s.first ms })
            last: (if $s.last == null { null } else { epoch $s.last ms })
            timed: ($s.timed | default 0) failures: ($s.failures | default 0)
        }
        txt: {
            path: $txt exists: ($txt | path exists)
            lines: (if ($txt | path exists) { open --raw $txt | lines | where $it != "" | length } else { 0 })
        }
    }
}

def nu-env [repo: string, scope: record, modules: list<record>]: nothing -> record {
    let dir = nu-config-dir
    let real_repo = $repo | path expand
    let bins = which -a nu | where type == external | get path | uniq | enumerate | each {|b|
        let r = ^$b.item --version | complete
        {path: $b.item, version: (if $r.exit_code == 0 { $r.stdout | str trim } else { null }), source: (nu-bin-source $b.item), active: ($b.index == 0)}
    }
    let plugin_path = $scope.plugin_path? | default ($dir | path join plugin.msgpackz)
    let config = $NU_CONFIG_FILES | each {|f| {name: $f, path: ($dir | path join $f)} }
        | append {name: "plugin.msgpackz", path: $plugin_path}
        | each {|c|
            let link = try { (ls -l $c.path | get 0.type) == symlink } catch { false }
            let target = if $link and ($c.path | path exists) { $c.path | path expand } else { null }
            {
                name: $c.name path: $c.path target: $target exists: ($c.path | path exists)
                managed: ($target | default $c.path | path expand | str starts-with $real_repo)
            }
        }
    let cmds = $scope.commands? | default []
    {
        version: ($bins | where active | get -o 0.version | default (version).version)
        binaries: $bins
        config: $config
        lib_dirs: ($scope.lib_dirs? | default [] | each {|d| {
            path: $d exists: ($d | path exists)
            modules: ($modules | where {|m| $m.origin == "lib-dir" and ($m.path | path dirname) == $d } | get name)
        } })
        plugin_path: $plugin_path
        plugins: ($scope.plugins? | default [])
        history: (nu-history-stats $scope)
        counts: (if ($scope | is-empty) { null } else { {
            "built-in": ($cmds | where type == built-in | length)
            keyword: ($cmds | where type == keyword | length)
            plugin: ($cmds | where type == plugin | length)
            custom: ($cmds | where type == custom | length)
            alias: ($scope.aliases? | default [] | length)
        } })
    }
}

# nu-native heads and nu aliases in nu history, as nu resolves them (scope
# aliases only: in nu, `ls` is the built-in).
def nu-commands [parsed: list<record>, native: record, aliases: list<string>, cutoff: any]: nothing -> list<record> {
    let rows = $parsed | where src in [nd nt] | each {|p|
        let al = $p.heads | where {|w| $w.0 in $aliases } | each {|w| {name: $w.0, type: "alias", category: "alias"} }
        let nat = nu-heads $p.heads $native $aliases | each {|n| let c = $native | get $n; {name: $n, type: $c.type, category: $c.category} }
        $al ++ $nat | uniq-by name | each {|r| $r | merge {src: $p.src, ts: $p.ts, exit: $p.exit?, ms: $p.ms?} }
    } | flatten
    if ($rows | is-empty) { return [] }
    $rows | group-by name | transpose name rs | each {|g|
        {name: $g.name, type: $g.rs.0.type, category: $g.rs.0.category, uses: ($g.rs | length)} | merge (window-cost $g.rs $cutoff)
    } | sort-by uses --reverse
}

def nu-script-path [raw: string, cwd: any]: nothing -> any {
    let p = if ($raw | str starts-with "/") or ($raw | str starts-with "~") { $raw | path expand } else if $cwd != null { $cwd | path join $raw | path expand } else { null }
    if $p != null and ($p | path exists) { $p } else { null }
}

def nu-script-row [s: record, rows: list<record>, cutoff: any]: nothing -> record {
    let n = {|src| $rows | where src == $src | length }
    let u = shell-uses {zh: (do $n zh), zl: (do $n zl), nd: (do $n nd), nt: (do $n nt), bh: (do $n bh)}
    $s | merge $u | insert uses ($u.zsh + $u.nu + $u.bash) | merge (window-cost $rows $cutoff)
}

# Every .nu script: dotconfig config/scripts (run by bin name), dotconfig
# scripts/**, and any other .nu run in history (`nu x.nu`, `./x.nu`). A line
# runs a known script when it names its path (resolved against the nu sqlite
# cwd; absolute paths in any shell), or its bin/basename unless every such path
# resolved to another file. The rest group by basename.
def nu-scripts [repo: string, parsed: list<record>, cutoff: any]: nothing -> list<record> {
    let dot = glob $"($repo)/config/scripts/*.nu" | each {|f|
        let bin = $f | path basename | str replace -r '\.nu$' ''
        {name: $bin, path: ($f | path expand), origin: "dotconfig-script", keys: [$bin ($f | path basename)]}
    }
    let known = $dot ++ (glob $"($repo)/scripts/**/*.nu" | each {|f|
        {name: ($f | path basename), path: ($f | path expand), origin: "repo-script", keys: [($f | path basename)]}
    })
    let keyset = $known | get keys | flatten | uniq
    let runs = $parsed | enumerate | each {|e|
        let p = $e.item
        let refs = $p.scripts | where {|s| $s | str ends-with ".nu" }
        if ($refs | is-empty) and not ($p.tokens | any {|t| $t in $keyset }) { return null }
        {
            i: $e.index src: $p.src ts: $p.ts exit: $p.exit? ms: $p.ms? tokens: $p.tokens
            refs: ($refs | each {|s| {raw: $s, base: ($s | path basename), abs: (nu-script-path $s $p.cwd?)} })
        }
    } | compact
    let hits = $known | each {|k|
        let rows = $runs | each {|r|
            let same = $r.refs | where {|x| $x.base in $k.keys }
            let elsewhere = ($same | is-not-empty) and ($same | all {|x| $x.abs != null and $x.abs != $k.path })
            let named = $r.refs | any {|x| $x.abs == $k.path }
            if not ($named or (($r.tokens | any {|t| $t in $k.keys }) and not $elsewhere)) { return null }
            let mine = $r.refs | where {|x| $x.abs == $k.path or ($x.base in $k.keys and $x.abs == null) }
            $r | insert as (if ($mine | is-empty) { $r.tokens | where {|t| $t in $k.keys } } else { $mine | get raw })
            | insert res ($mine | get abs | compact)
            | insert claimed ($mine | each {|x| $"($r.i)\t($x.raw)" })
        } | compact
        {k: $k, rows: $rows}
    }
    let claimed = $hits | each {|h| $h.rows | each {|r| $r.claimed } } | flatten | flatten
    let known_rows = $hits | each {|h|
        let s = {
            name: $h.k.name path: $h.k.path origin: $h.k.origin
            invoked_as: ($h.rows | each {|r| $r.as } | flatten | uniq)
            resolved: ($h.rows | each {|r| $r.res } | flatten | uniq)
        }
        nu-script-row $s $h.rows $cutoff
    }
    let free = $runs | each {|r|
        $r.refs | where {|x| $"($r.i)\t($x.raw)" not-in $claimed } | each {|x| $x | merge {i: $r.i, src: $r.src, ts: $r.ts, exit: $r.exit, ms: $r.ms} }
    } | flatten
    let history = if ($free | is-empty) { [] } else {
        $free | group-by base | transpose name refs | each {|g|
            let resolved = $g.refs | get abs | compact | uniq
            let s = {
                name: $g.name path: (if ($resolved | length) == 1 { $resolved.0 } else { null }) origin: "history"
                invoked_as: ($g.refs | get raw | uniq) resolved: $resolved
            }
            nu-script-row $s ($g.refs | uniq-by i) $cutoff
        }
    }
    $known_rows ++ $history | sort-by uses --reverse
}

# The NuReport (apps/toolbelt-dashboard, platform-dashboard read it as JSON).
def nu-report [since: any]: nothing -> record {
    let repo = repo-dir
    let cutoff = if $since == null { null } else { (date now) - $since }
    let jobs = [hist nu] | par-each --keep-order {|j| if $j == "hist" { invocations } else { nu-scope } }
    let scope = $jobs.1
    let native = nu-native $scope
    let aliases = try { open ($repo | path join config shell config.toml) | get -o aliases | default {} } catch { {} }
    let scope_aliases = $scope.aliases? | default [] | get name
    let parsed = parse-history $jobs.0 $aliases $scope $native
    let counts = aggregate ($parsed | each {|p| $p.tokens | each {|t| {src: $p.src, ts: $p.ts, key: $t} } } | flatten)
    let modules = nu-modules $repo $scope
    {
        generated_at: (date now)
        since: $cutoff
        env: (nu-env $repo $scope $modules)
        commands: (nu-commands $parsed $native $scope_aliases $cutoff)
        modules: ($modules | each {|m|
            let s = nu-stats $m.bins $counts
            let u = shell-uses $s
            $m | reject kind bins | insert uses ($u.zsh + $u.nu + $u.bash) | insert last $s.last
        })
        scripts: (nu-scripts $repo $parsed $cutoff)
    }
}

# nu itself: binaries on PATH, config wiring, NU_LIB_DIRS, plugins, history;
# nu-native commands, modules and .nu scripts with runs, failures and time.
def "main nu" [
    --since: duration   # cost window (runs, failures, time); omitted = all history
    --json              # one JSON object (the dashboards' NuReport)
] {
    let r = nu-report $since
    if $json { return ($r | to json) }
    let e = $r.env
    let window = if $r.since == null { "all history" } else { $"runs since (fmt-date $r.since)" }
    print $"(ansi white_bold)toolbelt nu(ansi reset) (ansi dark_gray)· nu ($e.version) · cost: ($window) · (date now | format date '%Y-%m-%d %H:%M')(ansi reset)"

    header "BINARIES — every nu on PATH, first wins"
    print ($e.binaries | each {|b| {active: (if $b.active { $"(ansi green)●(ansi reset)" } else { "" }), version: ($b.version | default "?"), source: ($b.source | default "?"), path: (tilde $b.path)} } | table -i false)

    header "CONFIG"
    print ($e.config | each {|c| {
        name: $c.name exists: $c.exists
        managed: (if $c.managed { $"(ansi green)dotconfig(ansi reset)" } else { "" })
        path: (tilde $c.path) target: ($c.target | default "" | tilde $in)
    } } | table -i false)
    print ($e.lib_dirs | each {|d| {lib_dir: (tilde $d.path), exists: $d.exists, modules: ($d.modules | str join " ")} } | table -i false)
    if ($e.plugins | is-empty) {
        print $"(ansi dark_gray)no plugins registered \((tilde $e.plugin_path)\)(ansi reset)"
    } else {
        print ($e.plugins | each {|p| {plugin: $p.name, version: $p.version, status: $p.status, commands: ($p.commands | length), file: (tilde $p.filename)} } | table -i false)
    }
    let h = $e.history
    print $"   (ansi dark_gray)history (ansi reset)  ($h.format | default '?') · max ($h.max_size | default '?') · sqlite ($h.sqlite.rows) rows \(($h.sqlite.bytes | into filesize) + wal ($h.sqlite.wal_bytes | into filesize)\) (fmt-date $h.sqlite.first) → (fmt-date $h.sqlite.last) · ($h.sqlite.timed) timed · ($h.sqlite.failures) failed · txt ($h.txt.lines) lines"
    if $e.counts != null {
        let c = $e.counts
        print $"   (ansi dark_gray)scope   (ansi reset)  ($c.'built-in') built-in · ($c.keyword) keyword · ($c.plugin) plugin · ($c.custom) custom · ($c.alias) alias"
    }

    header "COMMANDS — nu-native heads and nu aliases in nu history"
    print ($r.commands | cost-view | table -i false)

    header "MODULES — NU_LIB_DIRS, init scripts, config defs"
    print ($r.modules | each {|m| {
        name: $m.name origin: $m.origin managed: $m.managed uses: $m.uses loc: $m.loc
        exports: ($m.exports | length) loaded_by: ($m.loaded_by | str join " ")
        "no effect": ($m.ineffective_loads | each {|c| $"(ansi yellow)($c)(ansi reset)" } | str join " ")
        last: (fmt-date $m.last) path: (tilde $m.path)
    } } | table -i false)
    if ($r.modules | any {|m| $m.ineffective_loads | is-not-empty }) {
        print $"(ansi dark_gray)no effect = loaded inside a block \(e.g. `if … { use … }`\): nu scopes `use`/`source` to that block, so nothing reaches the shell(ansi reset)"
    }

    header "SCRIPTS — every .nu script; runs/failures/time from the zsh run log + nu sqlite"
    print ($r.scripts | where uses > 0 | each {|s| $s
        | select name origin uses zsh nu bash runs timed failures total_ms avg_ms last
        | insert path ($s.path | default "" | tilde $in)
    } | cost-view | table -i false)
    let idle = $r.scripts | where uses == 0
    if ($idle | is-not-empty) { print $"(ansi dark_gray)never run: ($idle | get name | str join ' ')(ansi reset)" }
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
- managed: declared (listed in the repo's Brewfile / liner.toml / package.json / uv tools.txt) | drift (installed but undeclared, so a new machine will not get it) | local (cargo build from a path/git checkout: not declarable) | global, project (mise)
- also: "also X" = manager X ships the same bins but loses on PATH; "shadowed by X" = X wins, so this copy never runs.
- allowed: the only actions valid for this row. Never pick anything else.

Actions:
- install: install a declared-but-missing row.
- uninstall: remove it from the machine; a declared row is also removed from its declaration file.
- declare: add a drift row to its declaration file so every machine gets it.
- undeclare: drop a declaration without touching the machine (for example, a missing entry that is a typo or duplicates an installed row, such as a case mismatch).

Goals, in priority order:
1. One manager per binary. For every also/shadowed pair, keep the copy that wins PATH, or the declared one, and uninstall the other.
2. Declarations match the machine. Declare drift that is used, or that plausibly serves the editor, shell init, or another tool. Uninstall drift that is unused and serves nothing. For a missing row: install it if it looks wanted; undeclare it if it looks wrong.
3. Unused declared tools: uninstall only when clearly unneeded, for example 0 runs, no editor/init role, and superseded by another row. When unsure, leave it alone.
Brew rows of kind lib (no bins) are usually dependencies of other formulae; leave them unless clearly drift.
Skip rows that need no change. Give each action a one-line reason that cites the evidence (status, uses, last, also). The summary is 1-3 sentences.'#

# mise `project` rows are runtimes a repo pins in its own mise.toml — the one job
# mise keeps (AGENTS.md "Package ownership"). They are owned by that repo, not by
# dotconfig, so they're never manage candidates; `mise prune` handles unpinned ones.
def manage-candidates [tools: list<record>]: nothing -> list<record> {
    $tools | where {|t|
        $t.source in $MANAGEABLE and not ($t.source == "mise" and $t.managed == "project") and (
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

# OTEL_RESOURCE_ATTRIBUTES with caller=<name> appended: Claude Code copies these keys onto
# every metric/event it exports (~/.claude/settings.json env), so AI usage is attributable.
def otel-caller [name: string]: nothing -> string {
    [($env.OTEL_RESOURCE_ATTRIBUTES? | default "") $"caller=($name)"] | where $it != "" | str join ","
}

# Tokens and cost of one `claude -p --output-format json` reply. cost_usd is Claude Code's
# estimate at API list price (cost_basis "list"): on a claude.ai plan it is not what you pay.
def claude-usage [out: record]: nothing -> record {
    let u = $out.usage? | default {}
    let models = $out.modelUsage? | default {}
    {
        models: ($models | columns)
        input: ($u.input_tokens? | default 0)
        cache_read: ($u.cache_read_input_tokens? | default 0)
        cache_write: ($u.cache_creation_input_tokens? | default 0)
        output: ($u.output_tokens? | default 0)
        cost_usd: ($out.total_cost_usd? | default null)
        cost_basis: ($models | values | get -o 0.costBasis | default null)
        duration_ms: ($out.duration_ms? | default null)
    }
}

def kilo [n: int]: nothing -> string {
    if $n < 1000 { $"($n)" } else { $"($n / 1000 | math round -p 1)k" }
}

def usage-line [u: record]: nothing -> string {
    let total_in = $u.input + $u.cache_read + $u.cache_write
    let cost = if $u.cost_usd == null { "cost n/a" } else { $"≈$($u.cost_usd | math round -p 4) at API ($u.cost_basis | default 'list') price" }
    let secs = if $u.duration_ms == null { "" } else { $" · ($u.duration_ms / 1000 | math round -p 1)s" }
    $"($u.models | str join ',') · (kilo $total_in) in \((kilo $u.cache_read) cache read, (kilo $u.cache_write) cache write\) · (kilo $u.output) out · ($cost)($secs)"
}

# {plan, usage}: usage is null for omp, whose -p text reply carries none.
def ask-agent [agent: string, model: any, prompt: string]: nothing -> record {
    let m = if $model == null { [] } else { ["--model" $model] }
    match $agent {
        "claude" => {
            if not (has-cmd claude) { error make {msg: "claude CLI not found: install it or pass --agent omp"} }
            # Read-only tools stay available; StructuredOutput enforces the schema.
            let r = with-env {OTEL_RESOURCE_ATTRIBUTES: (otel-caller toolbelt-manage)} {
                $prompt | ^claude -p --no-session-persistence --output-format json --json-schema ($PLAN_SCHEMA | to json -r) --disallowedTools "Bash,Edit,Write,NotebookEdit,WebFetch,WebSearch,Task" ...$m | complete
            }
            let out = try { $r.stdout | from json } catch { error make {msg: $"claude failed \(exit ($r.exit_code)\): ($r.stderr | str trim)"} }
            if ($out.is_error? | default false) { error make {msg: $"claude: ($out.result? | default 'error'). Run `claude /login`, or pass --agent omp"} }
            {plan: ($out.structured_output? | default (json-in ($out.result? | default ""))), usage: (claude-usage $out)}
        }
        "omp" => {
            if not (has-cmd omp) { error make {msg: "omp CLI not found: install it or pass --agent claude"} }
            let r = $prompt | ^omp -p --no-tools --no-session ...$m | complete
            if $r.exit_code != 0 { error make {msg: $"omp failed \(exit ($r.exit_code)\): ($r.stderr | str trim)"} }
            {plan: (json-in $r.stdout), usage: null}
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
    let answer = ask-agent $agent $model (manage-prompt $rows)
    let reply = $answer.plan
    let usage = $answer.usage
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
            usage: $usage
        } | to json)
    }

    print $"(ansi white_bold)toolbelt manage(ansi reset) (ansi dark_gray)· ($agent) · ($rows | length) rows reviewed · (date now | format date '%Y-%m-%d %H:%M')(ansi reset)"
    if $usage != null { print $"(ansi dark_gray)usage: (usage-line $usage)(ansi reset)" }
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
# Lists every scanned cluster is asked for; CRD_LISTS only when the cluster serves them.
const CORE_LISTS = [namespaces pods clusterrolebindings networkpolicies validatingadmissionpolicybindings]
const CRD_LISTS = [
    clusters.cluster.x-k8s.io
    clusterpolicies.kyverno.io policies.kyverno.io
    ciliumnetworkpolicies.cilium.io
    kustomizations.kustomize.toolkit.fluxcd.io helmreleases.helm.toolkit.fluxcd.io
    gitrepositories.source.toolkit.fluxcd.io ocirepositories.source.toolkit.fluxcd.io
    providers.pkg.crossplane.io functions.pkg.crossplane.io configurations.pkg.crossplane.io
]
# Concurrent kubectl lists per cluster: fast, without flooding a small kind API server.
const KUBE_THREADS = 8

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
    let leak_f = [zsh nu bash] | each {|sh|
        let lines = $inv | where {|i| ($SHELL_OF | get $i.src) == $sh } | each {|i| $i.line } | uniq
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
    let fn_bodies = $cfg.functions? | default {} | transpose name f | each {|x| {name: $x.name, body: (fn-commands $x.f | str join "; ")} }
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

# A list fetched by cluster-objects; errors on a key it doesn't fetch, so a new
# check can't silently read nothing.
def items-of [o: record, key: string]: nothing -> list<any> {
    if $key not-in $o.items { error make {msg: $"cluster-objects doesn't fetch ($key)"} }
    $o.items | get $key
}

def label-of [meta: any, key: string]: nothing -> any {
    $meta.labels? | default {} | transpose k v | where k == $key | get -o 0.v
}

def ns-name [o: record]: nothing -> string { $"($o.metadata.namespace? | default '')/($o.metadata.name)" }

def floating-ref [ref: string]: nothing -> bool {
    not ($ref | str contains "@sha256:") and (($ref | str ends-with ":latest") or not ($ref | split row "/" | last | str contains ":"))
}

# name / kind / group / categories of every served API resource (CRDs and
# built-ins), from API discovery: kubectl caches it under ~/.kube/cache and it
# carries no OpenAPI schemas (listing CRDs ships every schema: seconds on
# Crossplane clusters). A partial discovery failure (a broken APIService) exits
# non-zero but still prints the rest.
def api-index [k: list<string>]: nothing -> list<record> {
    let r = ^kubectl ...$k --request-timeout=20s api-resources -o json | complete
    let res = try { $r.stdout | from json | get -o resources | default [] } catch { [] }
    $res | where {|a| ($a.group? | is-not-empty) and ("list" in ($a.verbs? | default [])) } | each {|a| {
        name: $"($a.name).($a.group)"
        kind: $a.kind
        group: $a.group
        cats: ($a.categories? | default [])
    } }
}

# Crossplane managed resources that are a whole cluster (GKE, EKS, AKS, …).
def xp-cluster-kind [c: record]: nothing -> bool {
    ("managed" in $c.cats) and (($c.kind in [KubernetesCluster ManagedCluster CivoKubernetes]) or (($c.kind == "Cluster") and ($c.group =~ '^(container\.gcp|eks\.aws|containerservice\.azure|kubernetes\.)')))
}

# Every list discover-children and cluster-checks read, fetched once and in
# parallel: {items: {key: items}, failed: [key]}. Keys are resource names plus the
# labeled queries; a CRD_LISTS entry the cluster doesn't serve is [] without a
# request. A failed list (timeout, forbidden) is [] too, and named in `failed`
# so its checks are reported blind instead of passing.
def cluster-objects [k: list<string>, apis: list<record>]: nothing -> record {
    let names = $apis | each {|a| $a.name }
    let dynamic = $apis | where {|a| (xp-cluster-kind $a) or ($a.name | str starts-with "providerconfigs.") } | each {|a| $a.name }
    let lists = $CORE_LISTS ++ ($CRD_LISTS | where {|r| $r in $names }) ++ $dynamic | uniq | each {|r| {key: $r, args: [get $r -A]} }
    let queries = $lists ++ ([
        {key: vcluster, args: [get "statefulsets,deployments" -A -l app=vcluster]}
        {key: argocd-clusters, args: [get secrets -A -l argocd.argoproj.io/secret-type=cluster]}
        {key: flux-controllers, args: [get deployments -A -l app.kubernetes.io/part-of=flux]}
        (if "constrainttemplates.templates.gatekeeper.sh" in $names { {key: constraints, args: [get constraints]} })
    ] | compact)
    let fetched = $queries | par-each -t $KUBE_THREADS {|q| {k: $q.key, v: (kube $k ...$q.args | get -o items)} }
    let absent = $CRD_LISTS | append constraints | each {|r| {k: $r, v: []} }
    {
        items: ($absent | transpose -r -d | merge ($fetched | each {|f| {k: $f.k, v: ($f.v | default [])} } | transpose -r -d))
        failed: ($fetched | where v == null | each {|f| $f.k } | sort)
    }
}

# Clusters this one created or deploys into: {name ns via secret server state spec}.
def discover-children [o: record, apis: list<record>]: nothing -> list<record> {
    let capi = items-of $o clusters.cluster.x-k8s.io | each {|c|
        let ep = $c.spec?.controlPlaneEndpoint?
        {
            name: $c.metadata.name, ns: $c.metadata.namespace, via: "capi"
            secret: {ns: $c.metadata.namespace, name: $"($c.metadata.name)-kubeconfig", key: "value"}
            server: (if ($ep.host? | is-empty) { null } else { $"https://($ep.host):($ep.port? | default 6443)" })
            state: ($c.status?.phase? | default "?"), spec: null
        }
    }
    let crossplane = $apis | where {|c| xp-cluster-kind $c } | each {|c|
        items-of $o $c.name | each {|m|
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
    let provider_configs = $apis | where {|c| ($c.name | str starts-with "providerconfigs.") and ($c.group =~ '^(kubernetes|helm)\.(m\.)?crossplane\.io$') }
        | each {|c|
            items-of $o $c.name | where {|p| $p.spec?.credentials?.source? == "Secret" } | each {|p|
                let r = $p.spec.credentials.secretRef
                {
                    name: $p.metadata.name, ns: ($p.metadata.namespace? | default ""), via: $"crossplane ($c.group | str replace -r '\.crossplane\.io$' '')/ProviderConfig"
                    secret: {ns: ($r.namespace? | default ($p.metadata.namespace? | default "crossplane-system")), name: $r.name, key: ($r.key? | default "kubeconfig")}
                    server: null, state: "", spec: null
                }
            }
        } | flatten
    let vclusters = items-of $o vcluster | each {|w|
        let n = label-of $w.metadata release | default $w.metadata.name
        {
            name: $n, ns: $w.metadata.namespace, via: "vcluster"
            secret: {ns: $w.metadata.namespace, name: $"vc-($n)", key: "config"}
            server: null, state: $"($w.status?.readyReplicas? | default 0)/($w.spec?.replicas? | default 1) ready", spec: null
        }
    }
    let flux = [kustomizations.kustomize.toolkit.fluxcd.io helmreleases.helm.toolkit.fluxcd.io] | each {|r|
        items-of $o $r | where {|x| $x.spec?.kubeConfig? != null } | each {|x|
            let s = $x.spec.kubeConfig.secretRef?
            let cm = $x.spec.kubeConfig.configMapRef?
            {
                name: ($s.name? | default ($cm.name? | default $x.metadata.name)), ns: $x.metadata.namespace, via: "flux kubeConfig"
                secret: (if $s == null { null } else { {ns: $x.metadata.namespace, name: $s.name, key: ($s.key? | default "value")} })
                server: null, state: "", spec: null
            }
        }
    } | flatten
    let argo = items-of $o argocd-clusters | each {|s|
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

def cluster-checks [o: record, t: string, apis: list<record>]: nothing -> list<record> {
    let nss = items-of $o namespaces | where {|n| $n.metadata.name not-in $SYSTEM_NS }
    let pods = items-of $o pods | where {|p| $p.metadata.namespace not-in $SYSTEM_NS }

    let kyverno = (items-of $o clusterpolicies.kyverno.io) ++ (items-of $o policies.kyverno.io)
    let kyverno_on = $kyverno | where {|p|
        (($p.spec?.validationFailureAction? | default "" | str lowercase) == "enforce") or ($p.spec?.rules? | default [] | any {|r| ($r.validate?.failureAction? | default "" | str lowercase) == "enforce" })
    }
    let gatekeeper = items-of $o constraints
    let gatekeeper_on = $gatekeeper | where {|c| ($c.spec?.enforcementAction? | default "deny") == "deny" }
    let vap = items-of $o validatingadmissionpolicybindings
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

    let admins = items-of $o clusterrolebindings | where {|b| $b.roleRef?.name? == "cluster-admin" }
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

    let cilium = items-of $o ciliumnetworkpolicies.cilium.io
    let segmented = (items-of $o networkpolicies) ++ $cilium | each {|p| $p.metadata.namespace? } | compact | uniq
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
    let flux_ctrl = items-of $o flux-controllers
    let flux_f = if ($flux_ctrl | is-empty) { [] } else {
        let default_sa = $flux_ctrl | any {|d| $d.spec.template.spec.containers | any {|c| $c.args? | default [] | any {|a| $a | str starts-with "--default-service-account" } } }
        let objs = [kustomizations.kustomize.toolkit.fluxcd.io helmreleases.helm.toolkit.fluxcd.io] | each {|r| items-of $o $r } | flatten
        let unscoped = $objs | where {|x| ($x.spec?.serviceAccountName? | is-empty) and ($x.spec?.kubeConfig? == null) }
        let sources = [gitrepositories.source.toolkit.fluxcd.io ocirepositories.source.toolkit.fluxcd.io] | each {|r| items-of $o $r } | flatten
        let unverified = $sources | where {|s| $s.spec?.verify? == null }
        [
            (finding clusters $t "agent inventory" info $"flux: ($flux_ctrl | each {|d| $d.metadata.name } | str join ' ')")
            (if (not $default_sa) and ($unscoped | is-not-empty) { finding clusters $t "agent least privilege" med $"($unscoped | length) Flux objects apply as the controller's cluster-admin SA: set serviceAccountName or --default-service-account" })
            (if ($unverified | is-not-empty) { finding clusters $t "signed sources" low $"($unverified | length) Flux sources pull without signature verification \(spec.verify\)" })
        ] | compact
    }
    let packages = [providers.pkg.crossplane.io functions.pkg.crossplane.io configurations.pkg.crossplane.io] | each {|r|
        items-of $o $r | each {|p| {kind: ($r | split row "." | first), name: $p.metadata.name, ref: ($p.spec?.package? | default "")} }
    } | flatten
    let unpinned = $packages | where {|p| floating-ref $p.ref }
    let pcs = $apis | where {|c| $c.name | str starts-with "providerconfigs." } | each {|c|
        items-of $o $c.name | each {|p| {group: $c.group, name: $p.metadata.name, source: ($p.spec?.credentials?.source? | default "none")} }
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
    let cov_f = if ($o.failed | is-empty) { [] } else {
        [(finding clusters $t "list coverage" med $"couldn't list ($o.failed | short-list) \(timeout or forbidden\): checks reading them are blind")]
    }
    [$policy_f] ++ $rbac_f ++ [$psa_f $np_f] ++ $pod_f ++ $flux_f ++ $xp_f ++ $cov_f
}

# GKE control-plane posture from gcloud, for gke_<project>_<location>_<name> contexts.
def gke-context-findings [label: string]: nothing -> list<record> {
    let gke = $label | parse -r '^gke_(?<project>[^_]+)_(?<location>[^_]+)_(?<name>.+)$' | get -o 0
    if ($gke == null) or not (has-cmd gcloud) { return [] }
    let d = gcloud-json container clusters describe $gke.name --location $gke.location --project $gke.project
    if $d == null {
        [(finding clusters $label "gke control plane" info "gcloud describe failed: no access, or the cluster is gone")]
    } else { gke-findings $label (gke-from-gcloud $d) }
}

# Reachability, controllers, children and checks for one cluster.
def scan-cluster [k: list<string>, label: string, server: any]: nothing -> record {
    # gcloud and the API server are independent: query them side by side.
    # Results are wrapped: par-each drops a bare null (an unreachable API server).
    let parts = [gke api] | par-each --keep-order {|j|
        if $j == "gke" { return {v: (gke-context-findings $label)} }
        let version = kube $k version | get -o serverVersion.gitVersion
        if $version == null { return {v: null} }
        let apis = api-index $k
        {v: {version: $version, apis: $apis, o: (cluster-objects $k $apis)}}
    }
    let gke_f = $parts.0.v
    let api = $parts.1.v
    if $api == null {
        return {label: $label, k: $k, server: $server, version: null, controllers: [], children: [], findings: ([(finding clusters $label "reachable" info "API server unreachable: in-cluster checks skipped")] ++ $gke_f)}
    }
    let names = $api.apis | each {|c| $c.name }
    let controllers = [
        [crossplane compositions.apiextensions.crossplane.io]
        [capi clusters.cluster.x-k8s.io]
        [flux kustomizations.kustomize.toolkit.fluxcd.io]
        [argocd applications.argoproj.io]
        [kyverno clusterpolicies.kyverno.io]
        [gatekeeper constrainttemplates.templates.gatekeeper.sh]
    ] | where {|p| $p.1 in $names } | each {|p| $p.0 }
    {
        label: $label, k: $k, server: $server, version: $api.version, controllers: $controllers
        children: (discover-children $api.o $api.apis)
        findings: ((cluster-checks $api.o $label $api.apis) ++ $gke_f)
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
