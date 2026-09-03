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
const HAND_WRITTEN_PACKAGES = ["zellij", "zed", "starship", "zsh", "nushell", "pnpm", "bun", "nvim"]

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
            if ($entry.value | str contains "'") {
                error make { msg: $"alias '($entry.key)' contains a single quote. Shell aliases are textual substitution — even properly escaped, the body re-parses at call time and fails. Move it to [functions.($entry.key)] in config.toml; the generated bash script handles embedded quotes correctly." }
            }
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

# nushell: aliases + env only. Alias values containing bash syntax ($(…), &&) become def blocks.
# Aliases targeting a [functions.*] bin script are emitted with a `^` prefix so they
# call the external script even when its name shadows a nu builtin (e.g. `update`).
def generate_nushell [config: record, output_dir: string] {
    reset_dir $output_dir
    let output_file = $output_dir | path join "generated.nu"

    mut content = "# Generated from config.toml — do not edit by hand.\n\n"

    if "aliases" in $config {
        $content = $content + "# Aliases\n"
        for entry in ($config.aliases | transpose key value) {
            let val = $entry.value
            if ($val | str contains "'") {
                error make { msg: $"alias '($entry.key)' contains a single quote, which can't be safely emitted as a bare nushell alias. Move it to [functions.($entry.key)] in config.toml — bash handles embedded quotes via the generated script on PATH." }
            }
            let has_subshell = $val | str contains '$('
            let has_andand = $val | str contains '&&'
            let fn_names = if "functions" in $config { $config.functions | columns } else { [] }
            let first_word = $val | split row " " | first

            if $has_subshell or $has_andand {
                $content = $content + $"export def ($entry.key) [] {\n"
                let converted = $val
                    | str replace -a '&&' ';'
                    | str replace -r '\$\(([^)]+)\)' '(^$1 | str trim)'
                let commands = $converted | split row ';' | each {|cmd| $cmd | str trim}
                for cmd in $commands {
                    $content = $content + $"    ^($cmd)\n"
                }
                $content = $content + "}\n"
            } else if $first_word in $fn_names {
                $content = $content + $"export alias ($entry.key) = ^($val)\n"
            } else {
                $content = $content + $"export alias ($entry.key) = ($val)\n"
            }
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
        let commands = if "commands" in $func {
            $func.commands
        } else if "command" in $func {
            [$func.command]
        } else {
            []
        }
        let on_error = $func | get -o on_error | default "abort"
        if $on_error not-in ["abort", "continue"] {
            error make { msg: $"functions.($name): on_error = '($on_error)' is not supported; use \"abort\" \(the default) or \"continue\"." }
        }

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
    if not ($scripts_dir | path exists) {
        return
    }

    mkdir $output_dir

    for file in (ls $scripts_dir | where type == file) {
        let src = $file.name
        let basename = $src | path basename
        # Skip dotfiles and READMEs
        if ($basename | str starts-with ".") or ($basename | str lowercase) == "readme.md" {
            continue
        }
        let stem = $basename | path parse | get stem
        let dest = $output_dir | path join $stem

        cp $src $dest
        ^chmod +x $dest
        log info $"Installed user script: ($src) → ($dest)"
    }
}

# Remove dangling symlinks in target_dir that point into stale_dir (a now-empty source).
# Called after pruning output/bin so ~/.local/bin/ doesn't accumulate broken symlinks.
def remove_dangling_links [target_dir: string, source_dir: string] {
    if not ($target_dir | path exists) {
        return
    }
    for entry in (ls $target_dir | where type == symlink) {
        let resolved = try { $entry.name | path expand } catch { "" }
        # path expand on a dangling symlink still returns the would-be target
        if not ($resolved | path exists) {
            let link_target = (^readlink $entry.name | str trim)
            if ($link_target | str contains "dotconfig/output/bin") {
                log info $"Removing dangling symlink: ($entry.name) → ($link_target)"
                rm $entry.name
            }
        }
    }
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
    let config = load_config $config_path

    log info $"Loading config from: ($config_path)"

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
        generate_user_scripts ($scripts_dir | path expand) $bin_expanded
    }

    # Clear out symlinks in ~/.local/bin/ that point at executables we no longer emit.
    if ("bin" in $targets) or ("scripts" in $targets) {
        remove_dangling_links ("~/.local/bin" | path expand) $bin_expanded
    }

    log info "Generation complete"
}

def run_stow [stow_dir: string, target_dir: string, item: string, dry_run: bool] {
    let item_dir = $stow_dir | path join $item
    if not ($item_dir | path exists) {
        log warning $"Skipping ($item): directory not found at ($item_dir)"
        return
    }

    let stow_cmd = if $dry_run {
        $"stow --no-folding -d ($stow_dir) -t ($target_dir) --no -v ($item)"
    } else {
        $"stow --no-folding -d ($stow_dir) -t ($target_dir) -v ($item)"
    }

    log info $"Running: ($stow_cmd)"
    let result = (bash -c $stow_cmd | complete)

    if $result.exit_code != 0 {
        log error $"Failed to stow ($item): ($result.stderr)"
    } else {
        log info $"Successfully stowed ($item)"
        if ($result.stdout | str length) > 0 {
            print $result.stdout
        }
    }
}

def run_unstow [stow_dir: string, target_dir: string, item: string, dry_run: bool] {
    let item_dir = $stow_dir | path join $item
    if not ($item_dir | path exists) {
        log warning $"Skipping ($item): directory not found at ($item_dir)"
        return
    }

    let stow_cmd = if $dry_run {
        $"stow -D -d ($stow_dir) -t ($target_dir) --no -v ($item)"
    } else {
        $"stow -D -d ($stow_dir) -t ($target_dir) -v ($item)"
    }

    log info $"Running: ($stow_cmd)"
    let result = (bash -c $stow_cmd | complete)

    if $result.exit_code != 0 {
        log error $"Failed to unstow ($item): ($result.stderr)"
    } else {
        log info $"Successfully unstowed ($item)"
        if ($result.stdout | str length) > 0 {
            print $result.stdout
        }
    }
}

export def "main stow" [
    --items: list<string> = []
    --dry-run
] {
    let repo_dir = $REPO_DIR | path expand
    let output_dir = $repo_dir | path join "output"
    let target_dir = "~" | path expand

    if not ($output_dir | path exists) {
        error make { msg: $"Output directory not found: ($output_dir)\nRun 'main generate' first." }
    }

    # Generated packages live under output/; hand-written ones at the repo root.
    let generated = ls $output_dir | where type == dir | get name | path basename
    let hand_written = $HAND_WRITTEN_PACKAGES | where ($it in (ls $repo_dir | where type == dir | get name | path basename))

    let selected_generated = if ($items | is-empty) { $generated } else { $items | where ($it in $generated) }
    let selected_hand_written = if ($items | is-empty) { $hand_written } else { $items | where ($it in $hand_written) }

    log info $"Stow target: ($target_dir)"
    log info $"Generated packages: ($selected_generated | str join ', ')"
    log info $"Hand-written packages: ($selected_hand_written | str join ', ')"

    for item in $selected_generated {
        run_stow $output_dir $target_dir $item $dry_run
    }
    for item in $selected_hand_written {
        run_stow $repo_dir $target_dir $item $dry_run
    }

    log info "Stow apply complete"
}

export def "main unstow" [
    --items: list<string> = []
    --dry-run
] {
    let repo_dir = $REPO_DIR | path expand
    let output_dir = $repo_dir | path join "output"
    let target_dir = "~" | path expand

    if not ($output_dir | path exists) {
        error make { msg: $"Output directory not found: ($output_dir)\nRun 'main generate' first." }
    }

    let generated = ls $output_dir | where type == dir | get name | path basename
    let hand_written = $HAND_WRITTEN_PACKAGES | where ($it in (ls $repo_dir | where type == dir | get name | path basename))

    let selected_generated = if ($items | is-empty) { $generated } else { $items | where ($it in $generated) }
    let selected_hand_written = if ($items | is-empty) { $hand_written } else { $items | where ($it in $hand_written) }

    log info $"Unstow target: ($target_dir)"
    log info $"Generated packages: ($selected_generated | str join ', ')"
    log info $"Hand-written packages: ($selected_hand_written | str join ', ')"

    for item in $selected_generated {
        run_unstow $output_dir $target_dir $item $dry_run
    }
    for item in $selected_hand_written {
        run_unstow $repo_dir $target_dir $item $dry_run
    }

    log info "Stow remove complete"
}
