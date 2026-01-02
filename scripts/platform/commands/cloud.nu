#!/usr/bin/env nu

# Cloud Provider Commands
# Unified interface to AWS, GCP, and Azure

use ../../../nu/shared/shared.nu [log]

const CLOUD_DIR = "scripts/platform/cloud"

# List available cloud providers
export def "main cloud list-providers" [] {
    let providers = [
        {
            name: "aws"
            description: "Amazon Web Services"
            configured: (check-aws)
            services: ["EC2" "S3" "Bedrock" "Secrets Manager"]
        }
        {
            name: "gcp"
            description: "Google Cloud Platform"
            configured: (check-gcp)
            services: ["Compute Engine" "Cloud Storage" "Vertex AI" "Secret Manager"]
        }
        {
            name: "azure"
            description: "Microsoft Azure"
            configured: (check-azure)
            services: ["VMs" "Blob Storage" "Azure OpenAI" "Key Vault"]
        }
    ]

    $providers | table
}

# Set active cloud provider
export def "main cloud use" [
    provider: string  # Provider name: aws, gcp, azure
] {
    if not ($provider in ["aws" "gcp" "azure"]) {
        log error $"Invalid provider: ($provider). Use aws, gcp, or azure."
        return
    }

    $env.CLOUD_PROVIDER = $provider
    log info $"Active provider set to: ($provider)"

    # Persist to config
    let config_file = $"($env.HOME)/.config/platform/cloud.json"
    mkdir ($config_file | path dirname)
    { active_provider: $provider } | save -f $config_file
}

# Get current active provider
export def "main cloud current" [] {
    let provider = get-active-provider
    log info $"Active provider: ($provider)"
    $provider
}

# Invoke AI model across providers
export def "main cloud ai invoke" [
    prompt: string               # Prompt to send
    --model(-m): string = ""     # Model to use
    --provider(-p): string = ""  # Override provider
] {
    let p = if $provider == "" { get-active-provider } else { $provider }

    let model = if $model == "" {
        get-default-model $p
    } else {
        $model
    }

    log info $"Using ($p) with model: ($model)"

    match $p {
        "aws" => { invoke-bedrock $model $prompt }
        "gcp" => { invoke-vertex $model $prompt }
        "azure" => { invoke-azure-openai $model $prompt }
        _ => { log error "Invalid provider" }
    }
}

# List storage buckets
export def "main cloud storage list" [
    --provider(-p): string = ""
] {
    let p = if $provider == "" { get-active-provider } else { $provider }

    match $p {
        "aws" => { aws s3 ls | lines }
        "gcp" => { gcloud storage buckets list --format="value(name)" | lines }
        "azure" => { az storage container list --query "[].name" -o tsv | lines }
        _ => { log error "Invalid provider" }
    }
}

# Upload file to cloud storage
export def "main cloud storage upload" [
    local_path: path    # Local file path
    remote_path: string # Remote path (bucket/key)
    --provider(-p): string = ""
] {
    let p = if $provider == "" { get-active-provider } else { $provider }

    log info $"Uploading ($local_path) to ($p):($remote_path)"

    match $p {
        "aws" => { aws s3 cp $local_path $"s3://($remote_path)" }
        "gcp" => { gcloud storage cp $local_path $"gs://($remote_path)" }
        "azure" => {
            let parts = $remote_path | split row "/"
            az storage blob upload --container-name ($parts.0) --name ($parts | skip 1 | str join "/") --file $local_path
        }
        _ => { log error "Invalid provider" }
    }
}

# Download file from cloud storage
export def "main cloud storage download" [
    remote_path: string # Remote path (bucket/key)
    local_path: path    # Local file path
    --provider(-p): string = ""
] {
    let p = if $provider == "" { get-active-provider } else { $provider }

    log info $"Downloading ($p):($remote_path) to ($local_path)"

    match $p {
        "aws" => { aws s3 cp $"s3://($remote_path)" $local_path }
        "gcp" => { gcloud storage cp $"gs://($remote_path)" $local_path }
        "azure" => {
            let parts = $remote_path | split row "/"
            az storage blob download --container-name ($parts.0) --name ($parts | skip 1 | str join "/") --file $local_path
        }
        _ => { log error "Invalid provider" }
    }
}

# Get secret from cloud secret store
export def "main cloud secret get" [
    name: string  # Secret name
    --provider(-p): string = ""
] {
    let p = if $provider == "" { get-active-provider } else { $provider }

    match $p {
        "aws" => {
            aws secretsmanager get-secret-value --secret-id $name --query SecretString --output text
        }
        "gcp" => {
            gcloud secrets versions access latest --secret $name
        }
        "azure" => {
            az keyvault secret show --vault-name $env.AZURE_KEYVAULT_NAME --name $name --query value -o tsv
        }
        _ => { log error "Invalid provider" }
    }
}

# Set secret in cloud secret store
export def "main cloud secret set" [
    name: string   # Secret name
    value: string  # Secret value
    --provider(-p): string = ""
] {
    let p = if $provider == "" { get-active-provider } else { $provider }

    match $p {
        "aws" => {
            aws secretsmanager put-secret-value --secret-id $name --secret-string $value
        }
        "gcp" => {
            echo $value | gcloud secrets versions add $name --data-file=-
        }
        "azure" => {
            az keyvault secret set --vault-name $env.AZURE_KEYVAULT_NAME --name $name --value $value
        }
        _ => { log error "Invalid provider" }
    }
}

# List compute instances
export def "main cloud compute list" [
    --provider(-p): string = ""
] {
    let p = if $provider == "" { get-active-provider } else { $provider }

    match $p {
        "aws" => {
            aws ec2 describe-instances --query "Reservations[].Instances[].{ID:InstanceId,Name:Tags[?Key=='Name'].Value|[0],State:State.Name,Type:InstanceType}" --output table
        }
        "gcp" => {
            gcloud compute instances list
        }
        "azure" => {
            az vm list --query "[].{Name:name,State:powerState,Size:hardwareProfile.vmSize}" -o table
        }
        _ => { log error "Invalid provider" }
    }
}

# Helper functions

def get-active-provider [] {
    if ($env.CLOUD_PROVIDER? | is-not-empty) {
        $env.CLOUD_PROVIDER
    } else {
        let config_file = $"($env.HOME)/.config/platform/cloud.json"
        if ($config_file | path exists) {
            (open $config_file).active_provider
        } else {
            "gcp"  # Default
        }
    }
}

def check-aws [] {
    ($env.AWS_ACCESS_KEY_ID? | is-not-empty) or ($"($env.HOME)/.aws/credentials" | path exists)
}

def check-gcp [] {
    ($env.GOOGLE_APPLICATION_CREDENTIALS? | is-not-empty) or (try { gcloud auth list 2>/dev/null | str contains "ACTIVE" } catch { false })
}

def check-azure [] {
    try { az account show 2>/dev/null | is-not-empty } catch { false }
}

def get-default-model [provider: string] {
    match $provider {
        "aws" => "anthropic.claude-3-sonnet-20240229-v1:0"
        "gcp" => "gemini-1.5-pro"
        "azure" => "gpt-4"
        _ => "gpt-4"
    }
}

def invoke-bedrock [model: string, prompt: string] {
    let payload = {
        anthropic_version: "bedrock-2023-05-31"
        max_tokens: 4096
        messages: [{ role: "user", content: $prompt }]
    }

    aws bedrock-runtime invoke-model --model-id $model --body ($payload | to json) --content-type "application/json" --accept "application/json" /dev/stdout | from json | get content.0.text
}

def invoke-vertex [model: string, prompt: string] {
    # Use gcloud for Vertex AI
    let project = $env.GCP_PROJECT
    let location = $env.GCP_LOCATION? | default "us-central1"

    let payload = {
        contents: [{ role: "user", parts: [{ text: $prompt }] }]
    }

    let url = $"https://($location)-aiplatform.googleapis.com/v1/projects/($project)/locations/($location)/publishers/google/models/($model):generateContent"

    http post $url $payload --headers [Authorization $"Bearer (gcloud auth print-access-token)"] | get candidates.0.content.parts.0.text
}

def invoke-azure-openai [model: string, prompt: string] {
    let endpoint = $env.AZURE_OPENAI_ENDPOINT

    let payload = {
        messages: [{ role: "user", content: $prompt }]
    }

    http post $"($endpoint)/openai/deployments/($model)/chat/completions?api-version=2024-02-01" $payload --headers [api-key $env.AZURE_OPENAI_KEY] | get choices.0.message.content
}

# Main entry point
def main [] {
    print "Cloud Provider Management"
    print ""
    print "Commands:"
    print "  cloud list-providers  - List available providers"
    print "  cloud use             - Set active provider"
    print "  cloud current         - Show current provider"
    print "  cloud ai invoke       - Invoke AI model"
    print "  cloud storage list    - List storage buckets"
    print "  cloud storage upload  - Upload file"
    print "  cloud storage download - Download file"
    print "  cloud secret get      - Get secret"
    print "  cloud secret set      - Set secret"
    print "  cloud compute list    - List compute instances"
}
