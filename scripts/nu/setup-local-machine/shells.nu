#!/usr/bin/env nu

use std log
#nu -c 'source scripts/nu/setup-local-machine/mcp.nu; main apply mcp --enable-playwright'
def load_config [config_path: string]: nothing -> record {
    if not ($config_path | path exists) {
        error make { msg: $"Config file not found: ($config_path)" }
    }
    open $config_path
}

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
            $content = $content + $"($entry.key)() {\n"

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
                    $content = $content + $"    ($processed)\n"
                }
            } else if "command" in $func {
                let processed = if ($func.command | str contains "{arg}") {
                    $func.command | str replace -a "{arg}" '$argv[1]'
                } else {
                    $func.command
                }
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

    if "aliases" in $config {
        $content = $content + "# Aliases\n"
        for entry in ($config.aliases | transpose key value) {
            $content = $content + $"export alias ($entry.key) = ($entry.value)\n"
        }
        $content = $content + "\n"
    }

    if "functions" in $config {
        $content = $content + "# Functions\n"
        for entry in ($config.functions | transpose key value) {
            let func = $entry.value

            if "description" in $func {
                $content = $content + $"# ($func.description)\n"
            }

            if "commands" in $func {
                let args = if "args" in $func {
                    $func.args | get 0
                } else {
                    "arg"
                }

                $content = $content + $"export def ($entry.key) [($args): string] {\n"
                for cmd in $func.commands {
                    let processed = if ($cmd | str contains "{arg}") {
                        $cmd | str replace -a "{arg}" $"$($args)"
                    } else {
                        $cmd
                    }
                    $content = $content + $"    ^($processed)\n"
                }
                $content = $content + "}\n\n"
            } else if "command" in $func {
                let args = if "args" in $func {
                    $func.args | get 0
                } else {
                    "arg"
                }

                let processed = if ($func.command | str contains "{arg}") {
                    $func.command | str replace -a "{arg}" $"$($args)"
                } else {
                    $func.command
                }

                $content = $content + $"export def ($entry.key) [($args): string] {\n"
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
    --zsh-dir: string = "~/dotconfig/zsh/.config/zsh"
    --fish-dir: string = "~/dotconfig/fish/.config/fish"
    --nu-dir: string = "~/dotconfig/nu/.config/nushell"
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

def "apply_atlas" [] {
    helm upgrade --install atlas-operator oci://ghcr.io/ariga/charts/atlas-operator --namespace atlas-operator --create-namespace --wait
}
