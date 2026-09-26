#!/usr/bin/env nu

use std/log

def load_config [config_path: string]: nothing -> record {
    if not ($config_path | path exists) {
        error make { msg: $"Config file not found: ($config_path)" }
    }
    open $config_path
}

def main [] {}

# Repo root, derived from this file's location: <repo>/scripts/nu/setup-local-machine/shells.nu
const REPO_DIR = path self | path dirname | path dirname | path dirname | path dirname
const REPO_NAME = $REPO_DIR | path basename

# Hand-written stow packages that live at the top of the repo (not under output/).
# output/ is for generator output only; anything you hand-edit goes here.
const HAND_WRITTEN_PACKAGES = ["zed", "starship", "zsh", "nushell", "pnpm", "bun", "nvim", "mise"]

# Packages the generator writes under output/. Anything else found there is stale
# (e.g. an old `bash` package) and is removed by `generate`.
const GENERATED_PACKAGES = ["zsh", "nu", "bin"]

const TOP_LEVEL_KEYS = ["aliases", "functions", "environment"]
const FUNCTION_KEYS = ["description", "command", "commands", "on_error"]

# Aliases are textual; nushell aliases can't carry shell syntax. Anything needing
# it belongs in [functions.*] (a bash script on PATH, identical in every shell).
const ALIAS_FORBIDDEN = ["'", '$(', '`', '&&', '||', ';', '|']

# Names that nushell resolves to a builtin/keyword before any alias of ours.
def nu_builtins []: nothing -> list<string> {
    help commands | where command_type in [built-in keyword] | get name
}

# Hand-written scripts that become ~/.local/bin/<stem>. Dotfiles and READMEs are skipped.
def script_files [scripts_dir: string]: nothing -> table<name: string, stem: string> {
    if not ($scripts_dir | path exists) {
        return []
    }
    ls $scripts_dir
        | where type == file
        | get name
        | where {|p|
            let base = $p | path basename
            not ($base | str starts-with ".") and ($base | str lowercase) != "readme.md"
        }
        | each {|p| { name: $p, stem: ($p | path parse | get stem) } }
}

# Every problem in config.toml + config/scripts/, not just the first. Anything the
# generator would otherwise emit silently wrong (typo'd keys, a script overwriting a
# function, shell syntax nushell can't alias) is an error here.
def config_errors [config: record, scripts_dir: string]: nothing -> list<string> {
    mut errors = []

    for key in ($config | columns | where $it not-in $TOP_LEVEL_KEYS) {
        $errors = $errors | append $"unknown top-level table [($key)]; allowed: ($TOP_LEVEL_KEYS | str join ', ')"
    }

    let aliases = $config | get -o aliases | default {}
    let functions = $config | get -o functions | default {}
    let environment = $config | get -o environment | default {}
    let scripts = script_files $scripts_dir
    let fn_names = $functions | columns

    for entry in ($aliases | transpose key value) {
        if ($entry.value | describe) != "string" {
            $errors = $errors | append $"aliases.($entry.key): value must be a string"
            continue
        }
        for token in $ALIAS_FORBIDDEN {
            if ($entry.value | str contains $token) {
                $errors = $errors | append $"aliases.($entry.key): contains `($token)`; aliases are textual and can't carry shell syntax into nushell. Move it to [functions.($entry.key)]."
                break
            }
        }
        if $entry.key in $fn_names {
            $errors = $errors | append $"aliases.($entry.key): same name as [functions.($entry.key)]"
        }
        if $entry.key in $scripts.stem {
            $errors = $errors | append $"aliases.($entry.key): same name as script config/scripts/($scripts | where stem == $entry.key | first | get name | path basename)"
        }
    }

    for entry in ($functions | transpose key value) {
        let name = $entry.key
        let func = $entry.value
        if ($func | describe | str starts-with "record") == false {
            $errors = $errors | append $"functions.($name): must be a table"
            continue
        }
        for key in ($func | columns | where $it not-in $FUNCTION_KEYS) {
            $errors = $errors | append $"functions.($name): unknown key `($key)`; allowed: ($FUNCTION_KEYS | str join ', ')"
        }
        let has_command = "command" in $func
        let has_commands = "commands" in $func
        if $has_command and $has_commands {
            $errors = $errors | append $"functions.($name): set `command` or `commands`, not both"
        } else if not ($has_command or $has_commands) {
            $errors = $errors | append $"functions.($name): needs `command` or `commands`"
        } else if $has_commands and (($func.commands | describe) !~ '^list<string>' or ($func.commands | is-empty)) {
            $errors = $errors | append $"functions.($name): `commands` must be a non-empty list of strings"
        } else if $has_command and ($func.command | describe) != "string" {
            $errors = $errors | append $"functions.($name): `command` must be a string"
        }
        let on_error = $func | get -o on_error | default "abort"
        if $on_error not-in ["abort", "continue"] {
            $errors = $errors | append $"functions.($name): on_error = '($on_error)' is not supported; use \"abort\" \(the default) or \"continue\"."
        }
        if $name in $scripts.stem {
            $errors = $errors | append $"functions.($name): same name as script config/scripts/($scripts | where stem == $name | first | get name | path basename), which would overwrite it in ~/.local/bin"
        }
    }

    for dup in ($scripts | group-by stem | transpose stem files | where ($it.files | length) > 1) {
        $errors = $errors | append $"config/scripts: ($dup.files.name | path basename | str join ' and ') both install as ~/.local/bin/($dup.stem)"
    }

    let env_errors = $environment | transpose key value | each {|entry|
        [{|k, v| zsh_env $k $v}, {|k, v| nu_env $k $v}] | each {|emit|
            try { do $emit $entry.key $entry.value | ignore; null } catch {|e| $e.msg }
        }
    } | flatten | compact
    $errors = $errors | append $env_errors

    $errors | uniq
}

def assert_valid [config: record, scripts_dir: string] {
    let errors = config_errors $config $scripts_dir
    if ($errors | is-not-empty) {
        error make { msg: $"config is invalid:\n  - ($errors | str join "\n  - ")" }
    }
}

# Wrap a string as a POSIX single-quoted literal (valid in zsh and bash).
# Closes-and-reopens to embed `'`: `it's me` → `'it'\''s me'`. Safe for any
# value, including shell metacharacters.
def sh_q [s: string]: nothing -> string {
    let escaped = $s | str replace -a "'" "'\\''"
    $"'($escaped)'"
}

# Wrap a string as a nushell raw string. r#'…'# has no escape rules; the only
# closing sequence is '# — vanishingly rare in shell values. Safe for env values.
def nu_q [s: string]: nothing -> string {
    $"r#'($s)'#"
}

# Whitelist of shell variables env values may reference. Anything outside this
# list is rejected at generation time so `$TYPO_NAME` can't sneak through.
const ENV_ALLOWED_REFS = ['HOME', 'DOTCONFIG_DIR']

def validate_env_refs [s: string, label: string]: nothing -> nothing {
    let refs = $s | parse -r '\$([A-Za-z_][A-Za-z0-9_]*)' | get capture0
    let bad = $refs | where { |v| $v not-in $ENV_ALLOWED_REFS }
    if ($bad | length) > 0 {
        error make { msg: $"($label): value '($s)' references unsupported variable $($bad | first); only $HOME and $DOTCONFIG_DIR are allowed in env values." }
    }
}

# Emit a zsh env value. Single-quoted (verbatim) by default; double-quoted when
# the value references $HOME/$DOTCONFIG_DIR so the shell expands them at source.
def zsh_env [key: string, val: any]: nothing -> string {
    let s = if ($val | describe) == "bool" {
        if $val { "true" } else { "false" }
    } else {
        $val
    }
    if ($s | str contains '$') {
        validate_env_refs $s $"environment.($key)"
        for ch in ['"' '`' '\\'] {
            if ($s | str contains $ch) {
                error make { msg: $"environment.($key): value '($s)' contains '($ch)', which conflicts with double-quoted shell expansion." }
            }
        }
        $"\"($s)\""
    } else {
        sh_q $s
    }
}

# Emit a nu env value. Raw string by default; interpolated $"…" with $env.X
# substitution when the value references $HOME/$DOTCONFIG_DIR.
def nu_env [key: string, val: any]: nothing -> string {
    if ($val | describe) == "bool" {
        if $val { "true" } else { "false" }
    } else {
        let s = $val
        if ($s | str contains '$') {
            validate_env_refs $s $"environment.($key)"
            for ch in ['"' '`' '\\' '(' ')'] {
                if ($s | str contains $ch) {
                    error make { msg: $"environment.($key): value '($s)' contains '($ch)', which conflicts with nu interpolation." }
                }
            }
            let converted = $s
                | str replace -a '$HOME' '($env.HOME)'
                | str replace -a '$DOTCONFIG_DIR' '($env.DOTCONFIG_DIR)'
            $"$\"($converted)\""
        } else {
            nu_q $s
        }
    }
}

# Reset a generator output dir to empty so runtime state (e.g. zsh's .zcompdump,
# .zsh_history, .zsh_sessions/) can never leak into a stow package and later
# collide on `stow`. output/ is generator-owned; only generated files belong here.
def reset_dir [dir: string] {
    if ($dir | path exists) {
        rm -rf $dir
    }
    mkdir $dir
}

# zsh: aliases + env only. Functions become bash scripts on PATH (see generate_bin_scripts).
def generate_zsh [config: record, output_dir: string] {
    reset_dir $output_dir
    let output_file = $output_dir | path join "generated.zsh"

    mut content = "# Generated from config.toml — do not edit by hand.\n\n"

    if "aliases" in $config {
        $content = $content + "# Aliases\n"
        for entry in ($config.aliases | transpose key value) {
            $content = $content + $"alias ($entry.key)=(sh_q $entry.value)\n"
        }
        $content = $content + "\n"
    }

    if "environment" in $config {
        $content = $content + "# Environment Variables\n"
        $content = $content + (": \"${DOTCONFIG_DIR:=$HOME/" + $REPO_NAME + "}\"\n")
        $content = $content + "export DOTCONFIG_DIR\n"
        for entry in ($config.environment | transpose key value) {
            $content = $content + $"export ($entry.key)=(zsh_env $entry.key $entry.value)\n"
        }
    }

    $content | save -f $output_file
    log info $"Generated zsh config: ($output_file)"
}

# nushell: aliases + env only. Alias bodies are plain commands (config_errors rejects
# shell syntax). Aliases targeting a [functions.*] bin script are emitted with a `^`
# prefix so they call the external script even when its name shadows a nu builtin
# (e.g. `update`). Aliases whose own name is a nu builtin (`ls`) are zsh-only:
# shadowing it would replace nu's structured command with text output.
def generate_nushell [config: record, output_dir: string] {
    reset_dir $output_dir
    let output_file = $output_dir | path join "generated.nu"

    mut content = "# Generated from config.toml — do not edit by hand.\n\n"

    if "aliases" in $config {
        let fn_names = $config | get -o functions | default {} | columns
        let builtins = nu_builtins
        $content = $content + "# Aliases\n"
        for entry in ($config.aliases | transpose key value) {
            if $entry.key in $builtins {
                log info $"nu: skipping alias ($entry.key) — shadows the nu builtin; zsh only"
                continue
            }
            let first_word = $entry.value | split row " " | first
            let body = if $first_word in $fn_names { $"^($entry.value)" } else { $entry.value }
            $content = $content + $"export alias ($entry.key) = ($body)\n"
        }
        $content = $content + "\n"
    }

    if "environment" in $config {
        $content = $content + "# Environment Variables\n"
        $content = $content + ('$env.DOTCONFIG_DIR = ($env.DOTCONFIG_DIR? | default $"($env.HOME)/' + $REPO_NAME + '")' + "\n")
        for entry in ($config.environment | transpose key value) {
            $content = $content + $"$env.($entry.key) = (nu_env $entry.key $entry.value)\n"
        }
    }

    $content | save -f $output_file
    log info $"Generated nushell config: ($output_file)"
}

# Preamble for `on_error = "continue"` scripts: a `step` runner that executes a
# command, records it on failure, and keeps going.
# NOTE: a raw string may not open with `#` (`r#'#…` mis-lexes), hence the
# leading newlines and the comment line built as a normal string.
def step_runner [total: int]: nothing -> string {
    let body = r#'
STEP_BLUE='\033[1;34m'
STEP_RED='\033[0;31m'
STEP_NC='\033[0m'
step_no=0
failed_steps=()

step() {
    step_no=$((step_no + 1))
    printf "\n${STEP_BLUE}==> [%d/%d]${STEP_NC} %s\n" "$step_no" "$total_steps" "$1"
    # Capture on the || side: `fi` would reset $? to 0 before we could read it.
    local status=0
    eval "$1" || status=$?
    if ((status == 0)); then
        return 0
    fi
    printf "${STEP_RED}✗ step %d/%d failed (exit %d)${STEP_NC}: %s\n" "$step_no" "$total_steps" "$status" "$1" >&2
    failed_steps+=("$1")
}

'#
    let head = "# on_error = \"continue\": every command runs even if an earlier one fails.\n"
    $"($head)total_steps=($total)($body)"
}

# Epilogue for `on_error = "continue"` scripts: re-report the failures so they
# aren't buried thousands of lines up, and exit non-zero.
const STEP_SUMMARY = r#'
if ((${#failed_steps[@]} > 0)); then
    printf "\n${STEP_RED}%d of %d steps failed:${STEP_NC}\n" "${#failed_steps[@]}" "$total_steps" >&2
    for cmd in "${failed_steps[@]}"; do
        printf "  %s\n" "$cmd" >&2
    done
    exit 1
fi
'#

# Emit one bash script per [functions.*] under output/bin/.local/bin/<name>.
# These end up on PATH via stow and are callable from every shell.
#
# Default is `set -e`: the first failing command aborts the script. A function
# may set `on_error = "continue"` (see [functions.update]) to run every command
# and fail at the end instead — what a machine refresher wants, since one flaky
# upstream (rustup self-update, an expired gcloud token) otherwise skips every
# remaining step.
def generate_bin_scripts [config: record, output_dir: string] {
    if not ("functions" in $config) {
        return
    }

    mkdir $output_dir

    for entry in ($config.functions | transpose key value) {
        let name = $entry.key
        let func = $entry.value
        let script_path = $output_dir | path join $name
        let commands = if "commands" in $func { $func.commands } else { [$func.command] }
        let on_error = $func | get -o on_error | default "abort"

        mut content = "#!/usr/bin/env bash\n"
        $content = $content + "# Generated from config.toml — do not edit by hand.\n"
        if $on_error == "continue" {
            # Must precede the first command to apply file-wide: step bodies are
            # single-quoted on purpose, `eval` expands them at step time.
            $content = $content + "# shellcheck disable=SC2016\n"
        }
        # errexit is pointless under `continue`; the step runner owns failures.
        $content = $content + (if $on_error == "continue" { "set -uo pipefail\n" } else { "set -euo pipefail\n" })
        $content = $content + $"DOTCONFIG_DIR=\"${DOTCONFIG_DIR:-$HOME/($REPO_NAME)}\"\n"
        if "description" in $func {
            $content = $content + $"# ($func.description)\n"
        }
        $content = $content + "\n"

        if $on_error == "continue" {
            $content = $content + (step_runner ($commands | length))
            for cmd in $commands {
                $content = $content + $"step (sh_q $cmd)\n"
            }
            $content = $content + $STEP_SUMMARY
        } else {
            for cmd in $commands {
                $content = $content + $"($cmd)\n"
            }
        }

        $content | save -f $script_path
        ^chmod +x $script_path
        log info $"Generated bin script: ($script_path)"
    }
}

# Copy each file from config/scripts/ to output/bin/.local/bin/<name-without-extension>.
# These are hand-written scripts in any language (nu, bash, python, …). The shebang in
# each file determines the interpreter; the extension is for editor support and gets stripped.
def generate_user_scripts [scripts_dir: string, output_dir: string] {
    mkdir $output_dir

    for script in (script_files $scripts_dir) {
        let dest = $output_dir | path join $script.stem
        cp $script.name $dest
        ^chmod +x $dest
        log info $"Installed user script: ($script.name) → ($dest)"
    }
}

# Where stowed links live. stow never deletes a link whose source file is gone (a
# file dropped from a package, a removed package, a pruned output/ dir), so those
# dangle until something sweeps them. Depth-limited roots avoid walking caches.
const LINK_ROOTS = [
    { path: "~", depth: 1 }
    { path: "~/.local/bin", depth: 1 }
    { path: "~/.cargo", depth: 1 }
    { path: "~/.config", depth: 8 }
]

# Symlinks under $HOME that point into this repo and no longer resolve.
def dangling_repo_links []: nothing -> list<string> {
    let repo = $REPO_DIR | path expand
    $LINK_ROOTS
        | each {|root| { path: ($root.path | path expand), depth: $root.depth } }
        | where {|root| $root.path | path exists }
        | each {|root| ^find $root.path -maxdepth $root.depth -type l | lines }
        | flatten
        | where {|link|
            let target = ^readlink $link | str trim
            let abs = if ($target | str starts-with "/") {
                $target
            } else {
                $link | path dirname | path join $target | path expand --no-symlink
            }
            ($abs | str starts-with $"($repo)/") and not ($abs | path exists)
        }
}

def remove_dangling_links [dry_run: bool] {
    for link in (dangling_repo_links) {
        if $dry_run {
            log info $"Would remove dangling symlink: ($link)"
        } else {
            log info $"Removing dangling symlink: ($link)"
            rm $link
        }
    }
}

# Check config.toml + config/scripts/ without writing anything. Exit 1 listing
# every problem; `generate` runs the same checks and refuses invalid config.
export def "main validate" [
    --config-path: string = $"($REPO_DIR)/config/shell/config.toml"
    --scripts-dir: string = $"($REPO_DIR)/config/scripts"
] {
    let errors = config_errors (load_config ($config_path | path expand)) ($scripts_dir | path expand)
    if ($errors | is-empty) {
        print "config OK"
        return
    }
    for e in $errors {
        print -e $"✗ ($e)"
    }
    exit 1
}

export def "main generate" [
    --config-path: string = $"($REPO_DIR)/config/shell/config.toml"
    --zsh-dir: string = $"($REPO_DIR)/output/zsh/.config/zsh"
    --nu-dir: string = $"($REPO_DIR)/output/nu/.config/nushell"
    --bin-dir: string = $"($REPO_DIR)/output/bin/.local/bin"
    --scripts-dir: string = $"($REPO_DIR)/config/scripts"
    --targets: list<string> = ["zsh", "nu", "bin", "scripts"]
] {
    let config_path = $config_path | path expand
    let scripts_dir = $scripts_dir | path expand
    let config = load_config $config_path

    log info $"Loading config from: ($config_path)"
    assert_valid $config $scripts_dir

    # output/ is generator-owned: drop packages we no longer generate, or stow
    # would keep linking them (a leftover `bash` package once collected history).
    let output_dir = $REPO_DIR | path join "output"
    if ($output_dir | path exists) {
        for stale in (ls $output_dir | where type == dir | where { ($in.name | path basename) not-in $GENERATED_PACKAGES }) {
            log info $"Removing stale generated package: ($stale.name)"
            rm -rf $stale.name
        }
    }

    if "zsh" in $targets {
        generate_zsh $config ($zsh_dir | path expand)
    }

    if "nu" in $targets {
        generate_nushell $config ($nu_dir | path expand)
    }

    # bin/scripts share output/bin/.local/bin/. Clean it first so renames and deletions
    # don't leave stale executables behind.
    let bin_expanded = $bin_dir | path expand
    if ("bin" in $targets) or ("scripts" in $targets) {
        if ($bin_expanded | path exists) {
            for f in (ls $bin_expanded | where type == file) {
                rm $f.name
            }
        }
    }

    if "bin" in $targets {
        generate_bin_scripts $config $bin_expanded
    }

    if "scripts" in $targets {
        generate_user_scripts $scripts_dir $bin_expanded
    }

    log info "Generation complete"
}

# Run one stow (or `stow -D`) and report success; callers aggregate failures.
def run_stow [stow_dir: string, target_dir: string, item: string, dry_run: bool, --delete]: nothing -> bool {
    let verb = if $delete { "unstow" } else { "stow" }
    let args = [--no-folding -d $stow_dir -t $target_dir -v]
        | append (if $delete { [-D] } else { [] })
        | append (if $dry_run { [--no] } else { [] })
        | append $item

    log info $"Running: stow ($args | str join ' ')"
    let result = ^stow ...$args | complete

    if $result.exit_code != 0 {
        log error $"Failed to ($verb) ($item): ($result.stderr | str trim)"
        return false
    }
    log info $"($verb) ok: ($item)"
    if ($result.stdout | str length) > 0 {
        print $result.stdout
    }
    true
}

# Resolve which packages to act on. Unknown package names and declared-but-missing
# packages are errors, not silent skips.
def select_packages [items: list<string>]: nothing -> table<dir: string, name: string> {
    let repo_dir = $REPO_DIR | path expand
    let output_dir = $repo_dir | path join "output"
    let all = ($GENERATED_PACKAGES | each {|p| { dir: $output_dir, name: $p } })
        | append ($HAND_WRITTEN_PACKAGES | each {|p| { dir: $repo_dir, name: $p } })

    let unknown = $items | where $it not-in $all.name
    if ($unknown | is-not-empty) {
        error make { msg: $"Unknown package\(s): ($unknown | str join ', '). Known: ($all.name | uniq | str join ', ')" }
    }
    let selected = if ($items | is-empty) { $all } else { $all | where name in $items }

    let missing = $selected | where {|p| not ($p.dir | path join $p.name | path exists) }
    if ($missing | is-not-empty) {
        let paths = $missing | each {|p| $p.dir | path join $p.name } | str join ', '
        error make { msg: $"Package dir\(s) missing: ($paths). Generated packages need `generate` first; hand-written ones must exist at the repo root." }
    }
    $selected
}

def stow_all [items: list<string>, dry_run: bool, delete: bool] {
    let target_dir = "~" | path expand
    let selected = select_packages $items
    log info $"Target: ($target_dir); packages: ($selected.name | str join ', ')"

    let failed = $selected | where {|p|
        if $delete {
            not (run_stow $p.dir $target_dir $p.name $dry_run --delete)
        } else {
            not (run_stow $p.dir $target_dir $p.name $dry_run)
        }
    }

    remove_dangling_links $dry_run

    if ($failed | is-not-empty) {
        error make { msg: $"stow failed for: ($failed.name | str join ', ')" }
    }
}

# Stow every package, or only the named ones: `shells.nu stow mise zsh`.
export def "main stow" [
    --dry-run
    ...items: string
] {
    stow_all $items $dry_run false
    log info "Stow apply complete"
}

export def "main unstow" [
    --dry-run
    ...items: string
] {
    stow_all $items $dry_run true
    log info "Stow remove complete"
}
