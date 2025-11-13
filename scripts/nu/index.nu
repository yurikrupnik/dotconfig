#!/usr/bin/env nu

use shared/shared.nu *

def main [] {}

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
            { name: $name, verbose: $verbose } | create
            istioctl install --set profile=ambient --skip-confirmation

            if (do --ignore-errors { kubectl get ns dbs --no-headers -o name | lines | length }) == 0 {
                ^kubectl create namespace dbs
                kompose convert --file ~/projects/playground/manifests/dockers/compose.yaml --namespace dbs --stdout | kubectl apply -f -
            }

            if (do --ignore-errors { kubectl -n flux-system get deployment source-controller -o name --no-headers | lines | length }) == 0 {
                ^gh auth token | ^flux bootstrap github --token-auth --owner=yurikrupnik --repository=gitops-v2 --branch=main --path=clusters/manager-cluster --personal --components-extra image-reflector-controller,image-automation-controller | complete
                sleep 1min
            }

            if (do --ignore-errors { kubectl get secret secret-puller --no-headers -o name | lines | length }) == 0 {
                kubectl create secret generic secret-puller --from-file=creds=$"($env.HOME)/dotconfig/tmp/secret-puller.json" -n default
                kubectl create secret generic secret-puller --from-file=creds=$"($env.HOME)/dotconfig/tmp/secret-puller.json" -n crossplane-system
                kubectl create ns apps
                kubectl create secret generic secret-puller --from-file=creds=$"($env.HOME)/dotconfig/tmp/secret-puller.json" -n apps
                kubectl create secret generic 1pass-puller --from-file=creds=$"($env.HOME)/dotconfig/tmp/1pass-sa.txt" -n default
            }

            kubectl -n crossplane-system wait deployment crossplane --for=condition=Available --timeout=180s
            kubectl -n external-secrets wait deployment external-secrets-webhook --for=condition=Available --timeout=180s
            kubectl -n external-secrets wait deployment external-secrets-cert-controller --for=condition=Available --timeout=180s

            if (do --ignore-errors { kubectl get secret gpc-docker-registry-secret --no-headers -o name | lines | length }) == 0 {
                kubectl create secret docker-registry gpc-docker-registry-secret --docker-server=europe-central2-docker.pkg.dev --docker-username=_json_key --docker-password="$(cat ./tmp/container-puller.json)" --docker-email=container-puller@sdp-demo-388112.iam.gserviceaccount.com
            }

            if (do --ignore-errors { kubectl get secret ghcr-credentials -n flux-system --no-headers -o name | lines | length }) == 0 {
                kubectl create secret docker-registry ghcr-credentials --namespace flux-system --docker-server docker.io --docker-username yurikrupnik --docker-password WAG0jech7jes-clic
            }

            if (do --ignore-errors { kubectl get secret iac-secrets -n crossplane-system --no-headers -o name | lines | length }) == 0 {
                kubectl apply -f ($env.HOME)/dotconfig/tmp/init.yaml
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
