#!/usr/bin/env nu

use std log
#nu -c 'source scripts/nu/setup-local-machine/mcp.nu; main apply mcp --enable-playwright'
def load_config [config_path: string]: nothing -> record {
    if not ($config_path | path exists) {
        error make { msg: $"Config file not found: ($config_path)" }
    }
    open $config_path
}

def main [] {}

def generate_zsh [config: record, output_dir: string] {
    mkdir $output_dir
    let output_file = $output_dir | path join "generated.zsh"

    mut content = "# Generated from config.toml\n\n"

    if "aliases" in $config {
        $content = $content + "# Aliases\n"
        for entry in ($config.aliases | transpose key value) {
            $content = $content + $"alias ($entry.key)='($entry.value)'\n"
        }
        $content = $content + "\n"
    }

    if "functions" in $config {
        $content = $content + "# Functions\n"
        for entry in ($config.functions | transpose key value) {
            let func = $entry.value
            $content = $content + $entry.key + "() {\n"

            if "commands" in $func {
                for cmd in $func.commands {
                    let processed = if ($cmd | str contains "{arg}") {
                        $cmd | str replace -a "{arg}" '$1'
                    } else {
                        $cmd
                    }
                    $content = $content + $"    ($processed)\n"
                }
            } else if "command" in $func {
                let processed = if ($func.command | str contains "{arg}") {
                    $func.command | str replace -a "{arg}" '$1'
                } else {
                    $func.command
                }
                $content = $content + $"    ($processed)\n"
            }

            $content = $content + "}\n\n"
        }
    }

    if "environment" in $config {
        $content = $content + "# Environment Variables\n"
        for entry in ($config.environment | transpose key value) {
            let val = if ($entry.value | describe) == "bool" {
                if $entry.value { "true" } else { "false" }
            } else {
                $entry.value
            }
            $content = $content + $"export ($entry.key)='($val)'\n"
        }
    }

    $content | save -f $output_file
    log info $"Generated zsh config: ($output_file)"
}

def generate_fish [config: record, output_dir: string] {
    mkdir $output_dir
    let functions_dir = $output_dir | path join "functions"
    mkdir $functions_dir

    if "aliases" in $config {
        let aliases_file = $output_dir | path join "generated_aliases.fish"
        mut content = "# Generated from config.toml\n\n"

        for entry in ($config.aliases | transpose key value) {
            $content = $content + $"alias ($entry.key) '($entry.value)'\n"
        }

        $content | save -f $aliases_file
        log info $"Generated fish aliases: ($aliases_file)"
    }

    if "functions" in $config {
        for entry in ($config.functions | transpose key value) {
            let func = $entry.value
            let func_file = $functions_dir | path join $"($entry.key).fish"
            mut content = "# Generated from config.toml\n"

            if "description" in $func {
                $content = $content + $"# ($func.description)\n"
            }

            $content = $content + $"\nfunction ($entry.key)\n"

            if "commands" in $func {
                for cmd in $func.commands {
                    let processed = if ($cmd | str contains "{arg}") {
                        $cmd | str replace -a "{arg}" '$argv[1]'
                    } else {
                        $cmd
                    }
                    # Replace bash "$@" with fish $argv
                    let processed = $processed | str replace -a '"$@"' '$argv'
                    $content = $content + $"    ($processed)\n"
                }
            } else if "command" in $func {
                let processed = if ($func.command | str contains "{arg}") {
                    $func.command | str replace -a "{arg}" '$argv[1]'
                } else {
                    $func.command
                }
                # Replace bash "$@" with fish $argv
                let processed = $processed | str replace -a '"$@"' '$argv'
                $content = $content + $"    ($processed)\n"
            }

            $content = $content + "end\n"
            $content | save -f $func_file
        }
        log info $"Generated fish functions: ($functions_dir)"
    }

    if "environment" in $config {
        let env_file = $output_dir | path join "generated_env.fish"
        mut content = "# Generated from config.toml\n\n"

        for entry in ($config.environment | transpose key value) {
            let val = if ($entry.value | describe) == "bool" {
                if $entry.value { "true" } else { "false" }
            } else {
                $entry.value
            }
            $content = $content + $"set -gx ($entry.key) '($val)'\n"
        }

        $content | save -f $env_file
        log info $"Generated fish environment: ($env_file)"
    }
}

def generate_nushell [config: record, output_dir: string] {
    mkdir $output_dir
    let output_file = $output_dir | path join "generated.nu"

    mut content = "# Generated from config.toml\n\n"

    # Build mapping of aliases that point to functions
    mut alias_to_function = {}
    if ("aliases" in $config) and ("functions" in $config) {
        let function_names = $config.functions | transpose key value | get key
        for entry in ($config.aliases | transpose key value) {
            if $entry.value in $function_names {
                $alias_to_function = ($alias_to_function | insert $entry.value $entry.key)
            }
        }
    }

    if "aliases" in $config {
        $content = $content + "# Aliases\n"
        for entry in ($config.aliases | transpose key value) {
            let val = $entry.value
            # Check if alias has bash-specific syntax that needs conversion to function
            let has_subshell = $val | str contains '$('
            let has_andand = $val | str contains '&&'
            let has_pipe = $val | str contains '|'
            # Check if this alias points to a function we're defining
            let points_to_function = $val in $alias_to_function

            if $points_to_function {
                # Skip - we'll use the alias name for the function itself
                continue
            } else if $has_subshell or $has_andand {
                # Convert to function for command substitution or multiple commands
                $content = $content + $"export def ($entry.key) [] {\n"
                # Convert bash $(...) to nushell (^...)
                let converted = $val
                    | str replace -a '&&' ';'
                    | str replace -r '\$\(([^)]+)\)' '(^$1 | str trim)'
                # Split by ; and execute each command
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

    if "functions" in $config {
        $content = $content + "# Functions\n"
        for entry in ($config.functions | transpose key value) {
            let func = $entry.value
            # Use alias name if this function has one pointing to it
            let func_name = if $entry.key in $alias_to_function {
                $alias_to_function | get $entry.key
            } else {
                $entry.key
            }

            if "description" in $func {
                $content = $content + $"# ($func.description)\n"
            }

            if "commands" in $func {
                # Check if any command uses "$@" to determine if we need rest parameters
                let uses_varargs = ($func.commands | any {|cmd| $cmd | str contains '"$@"' })
                # Check if any command uses {arg} placeholder
                let uses_arg = ($func.commands | any {|cmd| $cmd | str contains '{arg}' })

                let args = if "args" in $func {
                    $func.args | get 0
                } else {
                    "arg"
                }

                # Determine parameter signature based on actual usage
                let param_sig = if $uses_varargs {
                    "...args"
                } else if $uses_arg {
                    $"($args): string"
                } else {
                    ""  # No parameters needed
                }

                $content = $content + $"export def ($func_name) [($param_sig)] {\n"
                for cmd in $func.commands {
                    mut processed = if ($cmd | str contains "{arg}") {
                        $cmd | str replace -a "{arg}" $"$($args)"
                    } else {
                        $cmd
                    }
                    # Replace bash "$@" with nushell rest parameters
                    $processed = $processed | str replace -a '"$@"' '...$args'
                    $content = $content + $"    ^($processed)\n"
                }
                $content = $content + "}\n\n"
            } else if "command" in $func {
                # Check if command uses "$@" to determine if we need rest parameters
                let uses_varargs = $func.command | str contains '"$@"'
                # Check if command uses {arg} placeholder
                let uses_arg = $func.command | str contains '{arg}'

                let args = if "args" in $func {
                    $func.args | get 0
                } else {
                    "arg"
                }

                mut processed = if ($func.command | str contains "{arg}") {
                    $func.command | str replace -a "{arg}" $"$($args)"
                } else {
                    $func.command
                }

                # Replace bash "$@" with nushell rest parameters
                $processed = $processed | str replace -a '"$@"' '...$args'

                # Determine parameter signature based on actual usage
                let param_sig = if $uses_varargs {
                    "...args"
                } else if $uses_arg {
                    $"($args): string"
                } else {
                    ""  # No parameters needed
                }

                $content = $content + $"export def ($func_name) [($param_sig)] {\n"
                $content = $content + $"    ^($processed)\n"
                $content = $content + "}\n\n"
            }
        }
    }

    if "environment" in $config {
        $content = $content + "# Environment Variables\n"
        for entry in ($config.environment | transpose key value) {
            let val = if ($entry.value | describe) == "bool" {
                $entry.value
            } else {
                $"'($entry.value)'"
            }
            $content = $content + $"$env.($entry.key) = ($val)\n"
        }
    }

    $content | save -f $output_file
    log info $"Generated nushell config: ($output_file)"
}

export def "main generate" [
    --config-path: string = "~/dotconfig/scripts/nu/setup-local-machine/config.toml"
    --zsh-dir: string = "~/dotconfig/output/zsh/.config/zsh"
    --fish-dir: string = "~/dotconfig/output/fish/.config/fish"
    --nu-dir: string = "~/dotconfig/output/nu/.config/nushell"
    --shells: list<string> = ["zsh", "fish", "nu"]
] {
    let config_path = $config_path | path expand
    let config = load_config $config_path

    log info $"Loading config from: ($config_path)"

    if "zsh" in $shells {
        generate_zsh $config ($zsh_dir | path expand)
    }

    if "fish" in $shells {
        generate_fish $config ($fish_dir | path expand)
    }

    if "nu" in $shells {
        generate_nushell $config ($nu_dir | path expand)
    }

    log info "Shell configurations generated successfully"
}

# dampen stow output
export def "main stow" [
    --items: list<string> = []
    --dry-run
] {
    let output_dir = "~/dotconfig/output" | path expand
    let target_dir = "~" | path expand

    if not ($output_dir | path exists) {
        error make { msg: $"Output directory not found: ($output_dir)\nRun 'main generate' first." }
    }

    let all_items = if ($items | is-empty) {
        ls $output_dir | where type == dir | get name | path basename
    } else {
        $items
    }

    log info $"Applying shell configurations with stow from ($output_dir) to ($target_dir)"
    log info $"Items to stow: ($all_items | str join ', ')"

    for item in $all_items {
        let item_dir = $output_dir | path join $item
        if not ($item_dir | path exists) {
            log warning $"Skipping ($item): directory not found at ($item_dir)"
            continue
        }

        let stow_cmd = if $dry_run {
            $"stow -d ($output_dir) -t ($target_dir) --no -v ($item)"
        } else {
            $"stow -d ($output_dir) -t ($target_dir) -v ($item)"
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

    log info "Stow apply complete"
}

export def "main unstow" [
    --items: list<string> = []
    --dry-run
] {
    let output_dir = "~/dotconfig/output" | path expand
    let target_dir = "~" | path expand

    if not ($output_dir | path exists) {
        error make { msg: $"Output directory not found: ($output_dir)\nRun 'main generate' first." }
    }

    let all_items = if ($items | is-empty) {
        ls $output_dir | where type == dir | get name | path basename
    } else {
        $items
    }

    log info $"Removing shell configurations with stow from ($target_dir)"
    log info $"Items to unstow: ($all_items | str join ', ')"

    for item in $all_items {
        let item_dir = $output_dir | path join $item
        if not ($item_dir | path exists) {
            log warning $"Skipping ($item): directory not found at ($item_dir)"
            continue
        }

        let stow_cmd = if $dry_run {
            $"stow -D -d ($output_dir) -t ($target_dir) --no -v ($item)"
        } else {
            $"stow -D -d ($output_dir) -t ($target_dir) -v ($item)"
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

    log info "Stow remove complete"
}

def "main apply_atlas" [] {
    helm upgrade --install atlas-operator oci://ghcr.io/ariga/charts/atlas-operator --namespace atlas-operator --create-namespace --wait
}
