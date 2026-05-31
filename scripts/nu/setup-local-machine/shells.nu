#!/usr/bin/env nu

use std log

def load_config [config_path: string]: nothing -> record {
    if not ($config_path | path exists) {
        error make { msg: $"Config file not found: ($config_path)" }
    }
    open $config_path
}

def main [] {}

# Repo root, derived from this file's location: <repo>/scripts/nu/setup-local-machine/shells.nu
const REPO_DIR = path self | path dirname | path dirname | path dirname | path dirname

# Hand-written stow packages that live at the top of the repo (not under output/).
# output/ is for generator output only; anything you hand-edit goes here.
const HAND_WRITTEN_PACKAGES = ["zellij", "zed", "starship", "zsh", "nushell"]

# Wrap a string as a zsh single-quoted literal. Closes-and-reopens to embed `'`:
# `it's me` → `'it'\''s me'`. Safe for any value, including shell metacharacters.
def zsh_q [s: string]: nothing -> string {
    let escaped = $s | str replace -a "'" "'\\''"
    $"'($escaped)'"
}

# Wrap a string as a nushell raw string. r#'…'# has no escape rules; the only
# closing sequence is '# — vanishingly rare in shell values. Safe for env values.
def nu_q [s: string]: nothing -> string {
    $"r#'($s)'#"
}

# zsh: aliases + env only. Functions become bash scripts on PATH (see generate_bin_scripts).
def generate_zsh [config: record, output_dir: string] {
    mkdir $output_dir
    let output_file = $output_dir | path join "generated.zsh"

    mut content = "# Generated from config.toml — do not edit by hand.\n\n"

    if "aliases" in $config {
        $content = $content + "# Aliases\n"
        for entry in ($config.aliases | transpose key value) {
            if ($entry.value | str contains "'") {
                error make { msg: $"alias '($entry.key)' contains a single quote. Shell aliases are textual substitution — even properly escaped, the body re-parses at call time and fails. Move it to [functions.($entry.key)] in config.toml; the generated bash script handles embedded quotes correctly." }
            }
            $content = $content + $"alias ($entry.key)=(zsh_q $entry.value)\n"
        }
        $content = $content + "\n"
    }

    if "environment" in $config {
        $content = $content + "# Environment Variables\n"
        for entry in ($config.environment | transpose key value) {
            let val = if ($entry.value | describe) == "bool" {
                if $entry.value { "true" } else { "false" }
            } else {
                $entry.value
            }
            $content = $content + $"export ($entry.key)=(zsh_q $val)\n"
        }
    }

    $content | save -f $output_file
    log info $"Generated zsh config: ($output_file)"
}

# nushell: aliases + env only. Alias values containing bash syntax ($(…), &&) become def blocks.
def generate_nushell [config: record, output_dir: string] {
    mkdir $output_dir
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
            } else {
                $content = $content + $"export alias ($entry.key) = ($val)\n"
            }
        }
        $content = $content + "\n"
    }

    if "environment" in $config {
        $content = $content + "# Environment Variables\n"
        for entry in ($config.environment | transpose key value) {
            let val = if ($entry.value | describe) == "bool" {
                $entry.value
            } else {
                nu_q $entry.value
            }
            $content = $content + $"$env.($entry.key) = ($val)\n"
        }
    }

    $content | save -f $output_file
    log info $"Generated nushell config: ($output_file)"
}

# Emit one bash script per [functions.*] under output/bin/.local/bin/<name>.
# These end up on PATH via stow and are callable from every shell.
def generate_bin_scripts [config: record, output_dir: string] {
    if not ("functions" in $config) {
        return
    }

    mkdir $output_dir

    for entry in ($config.functions | transpose key value) {
        let name = $entry.key
        let func = $entry.value
        let script_path = $output_dir | path join $name

        let dotconfig = $REPO_DIR | path expand
        mut content = "#!/usr/bin/env bash\n"
        $content = $content + "# Generated from config.toml — do not edit by hand.\n"
        $content = $content + "set -euo pipefail\n"
        $content = $content + $"DOTCONFIG_DIR=\"${DOTCONFIG_DIR:-($dotconfig)}\"\n"
        if "description" in $func {
            $content = $content + $"# ($func.description)\n"
        }
        $content = $content + "\n"

        if "commands" in $func {
            for cmd in $func.commands {
                $content = $content + $"($cmd)\n"
            }
        } else if "command" in $func {
            $content = $content + $"($func.command)\n"
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
        if ($basename | str starts-with ".") or ($basename | str downcase) == "readme.md" {
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
