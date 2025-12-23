#!/usr/bin/env nu

use shared/shared.nu *
use local-dev/cluster.nu [create, delete_cluster]

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

            # Define all secret file paths
            let secret_puller_file = ($secret_dir | path join "secret-puller.json")
            let onepass_sa_file = ($secret_dir | path join "1pass-sa.txt")
            let container_puller_file = ($secret_dir | path join "container-puller.json")
            let init_yaml_file = ($secret_dir | path join "init.yaml")

            { name: $name, verbose: $verbose } | create
            istioctl install --set profile=ambient --skip-confirmation

            if (do --ignore-errors { kubectl get ns dbs --no-headers -o name | lines | length }) == 0 {
                ^kubectl create namespace dbs
                kompose convert --file ~/private/nx-playground/manifests/dockers/compose.yaml --namespace dbs --stdout | kubectl apply -f -
            }

            if (do --ignore-errors { kubectl -n flux-system get deployment source-controller -o name --no-headers | lines | length }) == 0 {
                # Use GITHUB_TOKEN from vals, fallback to gh CLI if not available
                let github_token = if ($env.GITHUB_TOKEN? | is-empty) {
                    log warning "GITHUB_TOKEN not found in vals secrets, using gh auth token"
                    (gh auth token)
                } else {
                    log info "Using GITHUB_TOKEN from vals secrets"
                    $env.GITHUB_TOKEN
                }

                with-env { GITHUB_TOKEN: $github_token } {
                    ^flux bootstrap github --owner=yurikrupnik --repository=gitops-v2 --branch=main --path=clusters/manager-cluster --personal --components-extra image-reflector-controller,image-automation-controller | complete
                }
                sleep 1min
            }

            if (do --ignore-errors { kubectl get secret secret-puller --no-headers -o name | lines | length }) == 0 {
                if not ($secret_puller_file | path exists) {
                    log error $"Secret file not found: ($secret_puller_file)"
                    return
                }
                if not ($onepass_sa_file | path exists) {
                    log error $"1Password SA file not found: ($onepass_sa_file)"
                    return
                }

                kubectl create secret generic secret-puller $"--from-file=creds=($secret_puller_file)" -n default
                kubectl create secret generic secret-puller $"--from-file=creds=($secret_puller_file)" -n crossplane-system
                kubectl create ns apps
                kubectl create secret generic secret-puller $"--from-file=creds=($secret_puller_file)" -n apps
                kubectl create secret generic 1pass-puller $"--from-file=creds=($onepass_sa_file)" -n default
            }

            kubectl -n crossplane-system wait deployment crossplane --for=condition=Available --timeout=180s
            kubectl -n external-secrets wait deployment external-secrets-webhook --for=condition=Available --timeout=180s
            kubectl -n external-secrets wait deployment external-secrets-cert-controller --for=condition=Available --timeout=180s

            if (do --ignore-errors { kubectl get secret gpc-docker-registry-secret --no-headers -o name | lines | length }) == 0 {
                if not ($container_puller_file | path exists) {
                    log error $"Container puller file not found: ($container_puller_file)"
                    return
                }

                let container_creds = (open $container_puller_file | str trim)
                kubectl create secret docker-registry gpc-docker-registry-secret --docker-server=europe-central2-docker.pkg.dev --docker-username=_json_key $"--docker-password=($container_creds)" --docker-email=container-puller@sdp-demo-388112.iam.gserviceaccount.com
            }

            if (do --ignore-errors { kubectl get secret ghcr-credentials -n flux-system --no-headers -o name | lines | length }) == 0 {
                if ($env.DOCKER_PASSWORD? | is-empty) {
                    log error "DOCKER_PASSWORD environment variable is not set. Please set it before running this command."
                    return
                }

                kubectl create secret docker-registry ghcr-credentials --namespace flux-system --docker-server docker.io --docker-username yurikrupnik $"--docker-password=($env.DOCKER_PASSWORD)"
            }

            if (do --ignore-errors { kubectl get secret iac-secrets -n crossplane-system --no-headers -o name | lines | length }) == 0 {
                if not ($init_yaml_file | path exists) {
                    log error $"Init YAML file not found: ($init_yaml_file)"
                    return
                }

                kubectl apply -f $"($init_yaml_file)"
            }

            kubectl wait "provider.pkg.crossplane.io/upbound-provider-gcp-storage" --for=condition=Installed --timeout=180s
            kubectl wait "provider.pkg.crossplane.io/upbound-provider-gcp-storage" --for=condition=Healthy --timeout=180s
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
