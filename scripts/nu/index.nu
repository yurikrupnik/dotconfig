#!/usr/bin/env nu

use shared/shared.nu *
use local-dev/cluster.nu [create, delete_cluster]
use local-dev/post-cluster.nu

# Fetch secrets from vals and return as a record
def fetch-secrets-from-vals [] {
    _require-bin "vals"

    log info "Fetching secrets from GCP Secret Manager via vals..."

    # Fetch secrets from vals and parse into a record
    let secrets_output = (^vals env -f ~/dotconfig/.vals.yaml)
    let secrets = ($secrets_output | lines)

    mut env_record = {}

    for line in $secrets {
        # Skip comments and empty lines
        if not ($line | str starts-with "#") and not ($line | str trim | is-empty) and ($line | str contains "=") {
            let parts = ($line | split row "=")
            if ($parts | length) >= 2 {
                let key = ($parts.0 | str trim)
                let value = ($parts | skip 1 | str join "=" | str trim)

                $env_record = ($env_record | insert $key $value)
            }
        }
    }

    log info $"✅ Fetched ($env_record | columns | length) secrets from vals"

    $env_record
}

def main [] {
    print "Development Environment Management"
    print "\nSecrets are automatically fetched from GCP Secret Manager via vals"
    print "No .env file is needed - secrets are loaded directly into memory"
    print "\nAvailable commands:"
    print "  main dev up      - Create and configure development cluster"
    print "  main dev down    - Delete development cluster"
    print "  main secrets     - Save secrets to .env file (optional)"
}

export def "main secrets" [] {
    _require-bin "vals"
    log info "Fetching secrets from GCP Secret Manager using vals..."
    ^vals env -f ~/dotconfig/.vals.yaml | save --force .env
    log info "✅ Secrets written to .env file"
    log warning "Remember: Never commit the .env file to version control!"
}

export def "main dev down" [
    --cloud: string = "local"
    --name(-n): string = "kind"
] {
    _validate-provider $cloud
    match $cloud {
        "aws" => {
            _require-bin "nu"
            _require-bin "aws"
        }
        "gcp" => {
            _require-bin "nu"
            _require-bin "gcloud"
        }
        "azure" => {
            _require-bin "nu"
            _require-bin "az"
        }
        "local" => {
            _require-bin "kind"
            delete_cluster $name
        }
    }
}

export def "main dev up" [
    --cloud: string = "local"
    --name(-n): string = "kind"
    --k8s-version: string = ""
    --enable-ingress
    --enable-ha = false
    --dry-run
    --verbose
] {
    # Load secrets directly from vals (no .env file needed)
    let secrets = (fetch-secrets-from-vals)
    load-env $secrets

    _validate-provider $cloud

    match $cloud {
        "aws" => {
            _require-bin "nu"
            _require-bin "aws"
        }
        "gcp" => {
            _require-bin "nu"
            _require-bin "gcloud"
        }
        "azure" => {
            _require-bin "nu"
            _require-bin "az"
        }
        "local" => {
            _require-bin "kind"
        }
        _ => {
            log error $"Unsupported cloud provider: ($cloud)"
            return
        }
    }

    let actions = {
        aws: {||
            log info "🟠 AWS cluster creation"
            if not $in.dry_run {
                nu scripts/cloud-providers.nu provider managed-cluster $CLOUD_PROVIDERS.aws
            }
        },
        gcp: {||
            log info "🔵 GCP cluster creation"
            if not $in.dry_run {
                nu scripts/cloud-providers.nu provider managed-cluster $CLOUD_PROVIDERS.gcp
            }
        },
        azure: {||
            log info "🟣 Azure cluster creation"
            if not $in.dry_run {
                nu scripts/cloud-providers.nu provider managed-cluster $CLOUD_PROVIDERS.azure
            }
        },
        local: {||
            # Configure secret directory with environment variable support
            let secret_dir = ($env.DOTCONFIG_SECRET_DIR? | default ($env.HOME | path join "dotconfig" "tmp"))

            let workers = if $enable_ha { 2 } else { 0 }
            { name: $name, verbose: $verbose, ingress: $enable_ingress, workers: $workers } | create

            # Use parallel post-cluster setup
            post-cluster setup all $secret_dir $secrets --flux-wait 30sec
        }
    }

    let ctx = {
        cluster_name: $name
        k8s_version: $k8s_version
        dry_run: $dry_run
        verbose: $verbose
    }

    let action = ($actions | get $cloud)
    do $action --env $ctx
    log info $"✅ Cluster flow finished."
}
