#!/usr/bin/env nu

def "main apply atlas" [] {

    print $"\nInstalling (ansi yellow_bold)Atlas Operator(ansi reset)...\n"

    (
        helm upgrade --install atlas-operator
            oci://ghcr.io/ariga/charts/atlas-operator
            --namespace atlas-operator --create-namespace
            --wait
    )

}

# Retrieves a container registry address
def "main get all_users" [] {
    if not ("./config.toml" | path exists) {
        print $"(ansi red)Error: config.toml file not found in current directory(ansi reset)"
        return null
    }

    let config = open ./config.toml
    let env_vars = $config.environment
    mut account_info = {}

    print $"(ansi yellow_bold)Retrieving cloud account information...(ansi reset)\n"

    let cloud_provider = $env_vars.CLOUD?

    # if $cloud_provider == "aws" or $cloud_provider == null {
    #     print $"(ansi blue)Checking AWS account...(ansi reset)"
    #     if (which aws | is-empty) {
    #         print $"  ❌ AWS CLI not installed"
    #         $account_info = ($account_info | upsert aws null)
    #     } else {
    #         try {
    #             let aws_identity = (aws sts get-caller-identity --output json | from json)
    #             $account_info = ($account_info | upsert aws {
    #                 account_id: $aws_identity.Account,
    #                 user_arn: $aws_identity.Arn,
    #                 user_id: $aws_identity.UserId,
    #                 region: ($env_vars.CLOUD_AREGION? | default "us-east-1")
    #             })
    #             print $"  ✅ AWS Account: ($aws_identity.Account)"
    #             print $"  ✅ AWS Region: ($account_info.aws.region)"
    #         } catch {
    #             print $"  ❌ AWS CLI not configured or not available"
    #             $account_info = ($account_info | upsert aws null)
    #         }
    #     }
    # }

    # if $cloud_provider == "gcp" or $cloud_provider == null {
    #     print $"(ansi blue)Checking GCP account...(ansi reset)"
    #     if (which gcloud | is-empty) {
    #         print $"  ❌ GCP CLI not installed"
    #         $account_info = ($account_info | upsert gcp null)
    #     } else {
    #         try {
    #             let config_project = $env_vars.CLOUD_GPROJECT?
    #             let config_account = $env_vars.CLOUD_GACOUNT?
    #             let config_region = $env_vars.CLOUD_GREGION?

    #             if $config_project != null {
    #                 gcloud config set project $config_project
    #             }
    #             if $config_account != null {
    #                 gcloud config set account $config_account
    #             }
    #             if $config_region != null {
    #                 gcloud config set compute/region $config_region
    #             }

    #             let gcp_config = (gcloud config list --format=json | from json)
    #             let gcp_account = $gcp_config.core.account?
    #             let gcp_project = $gcp_config.core.project?
    #             let gcp_region = $gcp_config.compute.region?

    #             if $gcp_account != null {
    #                 $account_info = ($account_info | upsert gcp {
    #                     account: $gcp_account,
    #                     project: $gcp_project,
    #                     region: $gcp_region
    #                 })
    #                 print $"  ✅ GCP Account: ($gcp_account)"
    #                 print $"  ✅ GCP Project: ($gcp_project)"
    #                 print $"  ✅ GCP Region: ($gcp_region)"
    #             } else {
    #                 print $"  ❌ GCP not configured"
    #                 $account_info = ($account_info | upsert gcp null)
    #             }
    #         } catch {
    #             print $"  ❌ GCP CLI not available"
    #             $account_info = ($account_info | upsert gcp null)
    #         }
    #     }
    # }

    # if $cloud_provider == "azure" or $cloud_provider == null {
    #     print $"(ansi blue)Checking Azure account...(ansi reset)"
    #     if (which az | is-empty) {
    #         print $"  ❌ Azure CLI not installed"
    #         $account_info = ($account_info | upsert azure null)
    #     } else {
    #         try {
    #             let azure_account = (az account show --output json | from json)
    #             $account_info = ($account_info | upsert azure {
    #                 subscription_id: $azure_account.id,
    #                 subscription_name: $azure_account.name,
    #                 tenant_id: $azure_account.tenantId,
    #                 user_name: $azure_account.user.name,
    #                 user_type: $azure_account.user.type,
    #                 region: ($env_vars.CLOUD_AREGION? | default "eastus")
    #             })
    #             print $"  ✅ Azure Subscription: ($azure_account.name) \(($azure_account.id)\)"
    #             print $"  ✅ Azure User: ($azure_account.user.name)"
    #             print $"  ✅ Azure Region: ($account_info.azure.region)"
    #         } catch {
    #             print $"  ❌ Azure CLI not configured or not available"
    #             $account_info = ($account_info | upsert azure null)
    #         }
    #     }
    # }

    # print $"\n(ansi green_bold)Account Information Summary:(ansi reset)"
    # print $"Primary Cloud Provider: ($cloud_provider)"
    # $account_info | table

    # $account_info
}

# Retrieves a container registry address
def "main get container_registry" [] {

    mut registry = ""
    if "CONTAINER_REGISTRY" in $env {
        $registry = $env.CONTAINER_REGISTRY
    } else {
        let value = input $"(ansi green_bold)Enter container image registry \(e.g., `ghcr.io/vfarcic`\):(ansi reset) "
        $registry = $value
    }
    $"CONTAINER_REGISTRY=($registry)\n" | save --append .env

    $registry

}

# Generate shell configurations from unified config
# --path: str = "~/configs-files/shells/config.toml"
# def main [
#     repo?: string = "../../../config.toml"
#     --path: string = "../../../config.toml"
# ] {
#     print $path
#     print $repo
#     # print $env
#     let config = open ($nu.env-paths.config-path | path join 'config.toml')
#     #let config = open ~/configs-files/shells/config.toml
#     #print $config
#     generate-zsh $config
#     #generate-fish $config
#     #generate-nu $config

#     print "✅ Generated configurations for all shells"
# }

def generate-zsh [config] {
    let zsh_dir = "~/dotconfig/zsh/.config/zsh"
    mkdir $zsh_dir
    mut content = "# Generated from shells/config.toml\n\n"
    # Aliases
    for alias in ($config.aliases | transpose key value) {
        $content = $content + $"alias ($alias.key)='($alias.value)'\n"
    }

    $content = $content + "\n"
    # Functions
    for func in ($config.functions | transpose key value) {
        $content = $content + $"($func.key)" + "() {\n"
        if "type" in $func.value and $func.value.type == "complex" {
            # Complex functions call Nu scripts with arguments
            $content = $content + $"    nu ($func.value.script) \"$@\"\n"
        } else if "commands" in $func.value {
            for cmd in $func.value.commands {
                let processed_cmd = ($cmd | str replace "{arg}" '$1')
                $content = $content + $"    ($processed_cmd)\n"
            }
        } else {
            let cmd = if "args" in $func.value {
                mut processed_cmd = $func.value.command
                for i in 0..($func.value.args | length) {
                    $processed_cmd = ($processed_cmd | str replace "{arg}" $"$($i + 1)")
                }
                $processed_cmd
            } else {
                ($func.value.command | str replace "{arg}" '$1')
            }
            $content = $content + $"    ($cmd)\n"
        }
        $content = $content + "}\n\n"
        print $content
    }

    $content | save --force $"($zsh_dir)/generated.zsh"
}

def generate-fish [config] {
    let fish_dir = ($env.HOME | path join "configs-files/fish/.config/fish")
    let functions_dir = ($fish_dir | path join "functions")
    mkdir $fish_dir
    mkdir $functions_dir

    # Create aliases file
    mut aliases_content = "# Generated from shells/config.toml\n\n"
    for alias in ($config.aliases | transpose key value) {
        $aliases_content = $aliases_content + $"alias ($alias.key) '($alias.value)'\n"
    }
    $aliases_content | save --force ($fish_dir | path join "generated_aliases.fish")

    # Create individual function files
    for func in ($config.functions | transpose key value) {
        mut func_content = $"# Generated from shells/config.toml\n"
        if "description" in $func.value {
            $func_content = $func_content + $"# ($func.value.description)\n"
        }
        $func_content = $func_content + $"\nfunction ($func.key)\n"

        if "type" in $func.value and $func.value.type == "complex" {
            # Complex functions call Nu scripts with arguments
            $func_content = $func_content + $"    nu ($func.value.script) $argv\n"
        } else if "commands" in $func.value {
            for cmd in $func.value.commands {
                let processed_cmd = ($cmd | str replace "{arg}" '$argv[1]')
                $func_content = $func_content + $"    ($processed_cmd)\n"
            }
        } else {
            let cmd = ($func.value.command | str replace "{arg}" '$argv[1]')
            $func_content = $func_content + $"    ($cmd)\n"
        }
        $func_content = $func_content + "end\n"

        $func_content | save --force ($functions_dir | path join $"($func.key).fish")
    }
}

def generate-nu [config] {
    let nu_dir = "~/configs-files/nu/.config/nu"
    mkdir $nu_dir

    mut content = "# Generated from shells/config.toml\n\n"

    # Aliases
    for alias in ($config.aliases | transpose key value) {
        $content = $content + $"export alias ($alias.key) = ($alias.value)\n"
    }

    $content = $content + "\n"

    # Functions
    for func in ($config.functions | transpose key value) {
        if "type" in $func.value and $func.value.type == "complex" {
            # Complex functions - keep original implementation or source from script
            $content = $content + $"# Complex function ($func.key) - use original implementation\n"
            $content = $content + $"# Or source from: ($func.value.script)\n\n"
        } else if "commands" in $func.value {
            let args = if "args" in $func.value { $func.value.args.0 } else { "arg" }
            $content = $content + $"export def ($func.key) [($args): string] {\n"
            for cmd in $func.value.commands {
                let processed_cmd = ($cmd | str replace "{arg}" $"$($args)")
                $content = $content + $"    ^($processed_cmd)\n"
            }
            $content = $content + "}\n\n"
        } else {
            let args = if "args" in $func.value { $func.value.args.0 } else { "arg" }
            let cmd = ($func.value.command | str replace "{arg}" $"$($args)")
            $content = $content + $"export def ($func.key) [($args): string] {\n"
            $content = $content + $"    ^($cmd)\n"
            $content = $content + "}\n\n"
        }
    }

    $content | save --force $"($nu_dir)/generated.nu"
}
