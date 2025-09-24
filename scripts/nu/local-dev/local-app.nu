use cluster.nu *

module greetings {
    export def hello [name: string] {
        $"hello ($name)!"
    }

    export def hi [where: string] {
        $"hi ($where)!"
    }
}
use greetings hello

def 'main local-app' [
    action: string = "local"
    --name (-n): string = "dev"
    --region (-r): string = "us-west1"

    # --action
] {
    print $"($action) command..."
    cluster-exists dev
    match $action {
        "gcp" => { create $name }
        "aws" => { cleanup_aws }
        #"azure" => { cleanup_azure }
        "local" => { cleanup_local }
        _ => { print $"No cleanup needed for provider: ($action)" }
    }
    string
    cluster-exists dev
    create dev

    # increment
    #kind create cluster

}

def 'main delete local-app' [
    --anthropic-api-key: string = "",
] {
    cluster-exists dev
    # create dev
    let data = main get anthropic
    let resolved_anthropic_api_key = if $anthropic_api_key != "" {
        $anthropic_api_key
    } else if ("ANTHROPIC_API_KEY" in $env) {
        $env.ANTHROPIC_API_KEY
    } else {
        ""
    }
    # increment
    #kind create cluster

}

def --env "main get anthropic" [] {

    mut anthropic_api_key = ""
    if "ANTHROPIC_API_KEY" in $env {
        $anthropic_api_key = $env.ANTHROPIC_API_KEY
    } else {
        let value = input $"(ansi green_bold)Enter Anthropic token:(ansi reset) "
        $anthropic_api_key = $value
    }
    $"export ANTHROPIC_API_KEY=($anthropic_api_key)\n" | save --append .env

    {token: $anthropic_api_key}

}
