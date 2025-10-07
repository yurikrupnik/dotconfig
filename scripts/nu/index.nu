#!/usr/bin/env nu
#
# use std json
#use std log
# use std path, toml yaml, json, rand, str path math fmt assert testing debug uuid
use local-dev/cluster.nu *
# source local-dev/index.nu
# source setup-local-machine/index.nu
# source shared/shared.nu
use shared/shared.nu *
# use local-dev/cluster.nu *

def main [] {}

def "main logger" [
    --shit
    --enable-playwright = false
    --memory-file-path: string = "",
    --shits: list<string> = [".mcp.json"],     # Enable Playwright MCP server for browser automation
] {
    use std assert
    assert equal (1 + 1) 2
    log critical "omg log type"
    log debug "omg log type"
    log warning "omg log type"
    log warning  "omg log type"
    log trace  "omg log type"
    # log custom "omg log type" ad
    log error "   brew install stow"
    log info $"log here ($shit)"
    log info $"log here ($shits)"
    log info $"log info ($enable_playwright)"
    let zshenv_path = $"($env.HOME)/.zshenv"
    log info $"🔍 DRY RUN: Would backup ($zshenv_path))"
    let resolved_memory_file_path = if $memory_file_path == "" {
        (pwd) | path join "memory.json" | path expand
        print $"pwd: (pwd)"
    } else {
        $memory_file_path
    }

    # log warn "omg log type"
}

export def "main dev down" [
    --cloud: string = "local"
    --name(-n): string = "kind"
] {
    _validate-provider $cloud
    match $cloud {
      "aws"   => { _require-bin "nu"; _require-bin "aws" }
      # "gcp"   => { _require-bin "nu"; _require-bin "gcloud" }
      # "azure" => { _require-bin "nu"; _require-bin "az" }
      "local" => { _require-bin "kind"; delete_cluster $name; }
      _ => {}
    }
}

export def "main dev up" [
    # --cluster: list<string> # do shit
    --cloud: string = "local"          # One of: aws, gcp, local, azure
    # --gitops: string = "flux"          # flux | argo | none (no install here; just pass-through for now)
    # --gcp-project: string = "playground-447016"
    --name(-n): string = "kind"
    # --ingress(-i): bool = fa
    --k8s-version: string = ""         # Optional, e.g. "v1.30.0"
    --enable-ingress  # Whether to enable ingress for the kind provider
    --enable-ha = false
    # --enable-ingress = true  # Whether to enable ingress for the kind provider
    --dry-run
    --verbose
] {
    # _validate-provider $cloud
    # with-env {FOO: "bar"} { echo $env.FOO }
    # with-env {RUST_LOG: "bar"} { echo $env.RUST_LOG }
    # Common dependency checks
    let local_dir = pwd | path basename
    print $"local: ($local_dir)"
    match $cloud {
      "aws"   => { _require-bin "nu"; _require-bin "aws" }
      # "gcp"   => { _require-bin "nu"; _require-bin "gcloud" }
      # "azure" => { _require-bin "nu"; _require-bin "az" }
      "local" => { _require-bin "kind" }
      _ => {
          log error $"Used not supported cloud provider: ($cloud)"
      }
    }
    #print $in
    #print $"enable-ha: ($enable_ha)"
    # if $verbose { log info $"Using temp file)" } else { log info $"Not using temp file" }
    # _log INFO $"🚀 Creating Kubernetes development cluster (cloud=($cloud), name=($cluster-name), gitops=($gitops))"

     # if $dry_run {
     #   _log WARN "Dry-run mode enabled — no changes will be made."
     # }
     # Dispatch table: per-provider action closures
      let actions = {
        aws:   {||
          log info "🟠 AWS cluster creation"
          if not $in.dry_run {
            nu scripts/cloud-providers.nu provider managed-cluster $CLOUD_PROVIDERS.aws
          }
        },
        gcp:   {||
          log info "🔵 GCP cluster creation"
          if not $in.dry_run {
            gcloud config set project $in.gcp_project | ignore
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
          {
              name: $name
              verbose: $verbose
          } | create
          istioctl install --set profile=ambient --skip-confirmation

          # let cmds = [
          #     ["istioctl install --set profile=ambient --skip-confirmation"]
          #   # ["kompose" "convert" "--file" "~/a.yaml" "-n" "dbs" "--stdout" "|" "kubectl" "apply" "-f" "-"] # (see note)
          #   # ["kubectl" "apply" "-f" "b.yaml"]
          #   # ["kubectl" "get" "pods" "-A"]
          #   # ["helm", "upgrade", "--install", "kyverno", "kyverno/kyverno", "--namespace kyverno", "--create-namespace", "--wait"]
          #   # ["^helm repo add kyverno https://kyverno.github.io/kyverno"]
          #   # ["^helm repo add external-secrets-operator https://charts.external-secrets.io/"]
          #   # ["helm", "upgrade", "--install", "my-external-secrets", "external-secrets-operator/external-secrets"]
          # ]

          # let jobs = ($cmds
          #   | par-each {|c|
          #       let res = (^($c.0) ...($c | skip 1)| complete)
          #       print $"c: ($c)"
          #       { cmd: ($c | str join ' '), exit: $res.exit_code, out: $res.stdout, err: $res.stderr }
          #   })
          # {name: "aris" verbose: $verbose}

#           let url = "https://raw.githubusercontent.com/yurikrupnik/gitops/main/cluster/cluster.yaml"
#           let cfg = (do { http get $url } catch {|e|
#             error make { msg: $"Failed to download cluster config from ($url): ($e.msg)" }
#           })
            # let da = True
          if (do --ignore-errors {kubectl get ns dbs --no-headers -o name | lines | length}) == 0 {
            ^kubectl create namespace dbs
            kompose convert --file ~/projects/playground/manifests/dockers/compose.yaml --namespace dbs --stdout | kubectl apply -f -
          }
          if (do --ignore-errors {kubectl -n flux-system get deployment source-controller -o name --no-headers | lines | length }) == 0 {
            ^gh auth token | ^flux bootstrap github --token-auth --owner=yurikrupnik --repository=gitops-v2 --branch=main --path=clusters/manager-cluster --personal --components-extra image-reflector-controller,image-automation-controller | complete
            sleep 1min
          }
          if (do --ignore-errors {kubectl get secret secret-puller --no-headers -o name | lines | length}) == 0 {
            kubectl create secret generic secret-puller --from-file=creds=$"($env.HOME)/dotconfig/tmp/secret-puller.json" -n default
            kubectl create secret generic secret-puller --from-file=creds=$"($env.HOME)/dotconfig/tmp/secret-puller.json" -n crossplane-system
            kubectl create ns apps
            kubectl create secret generic secret-puller --from-file=creds=$"($env.HOME)/dotconfig/tmp/secret-puller.json" -n apps
            kubectl create secret generic 1pass-puller --from-file=creds=$"($env.HOME)/dotconfig/tmp/1pass-sa.txt" -n default

          }
          kubectl -n crossplane-system wait deployment crossplane --for=condition=Available --timeout=180s
          kubectl -n external-secrets wait deployment external-secrets-webhook --for=condition=Available --timeout=180s
          kubectl -n external-secrets wait deployment external-secrets-cert-controller --for=condition=Available --timeout=180s

          # used by fluxcd in gitops-v2 github repo
          if (do --ignore-errors {kubectl get secret gpc-docker-registry-secret --no-headers -o name | lines | length}) == 0 {
              (
                  kubectl create secret docker-registry gpc-docker-registry-secret
                    --docker-server=europe-central2-docker.pkg.dev --docker-username=_json_key
                    --docker-password="$(cat ./tmp/container-puller.json)"
                    --docker-email=container-puller@sdp-demo-388112.iam.gserviceaccount.com
              )
          }


          # used by fluxcd in gitops-v2 github repo
          if (do --ignore-errors {kubectl get secret ghcr-credentials -n flux-system --no-headers -o name | lines | length}) == 0 {
              (
                  kubectl create secret docker-registry ghcr-credentials --namespace flux-system
                  --docker-server docker.io --docker-username yurikrupnik --docker-password WAG0jech7jes-clic
              )
          }
          # (
          #     kcl run ($env.HOME)/dotconfig/scripts/kcl/manager/main.k -D kind=Ars
          #       --path_selector items | kubectl apply -f -
          # )
          #
          #
          # This is for crossplane
          if (do --ignore-errors {kubectl get secret iac-secrets -n crossplane-system --no-headers -o name | lines | length}) == 0 {
              # kubectl create secret generic iac-manager-gcp --from-file=creds=$"($env.HOME)/dotconfig/tmp/iac-manager.yaml" -n crossplane-system
             kubectl apply -f ($env.HOME)/dotconfig/tmp/init.yaml
          }

          kubectl wait "provider.pkg.crossplane.io/upbound-provider-gcp-storage" --for=condition=Installed --timeout=180s
          kubectl wait "provider.pkg.crossplane.io/upbound-provider-gcp-storage" --for=condition=Healthy --timeout=180s

          # kubectl apply -f ($env.HOME)/dotconfig/tmp/temp.yaml # install bucket
          # kubectl port-forward service/konoplane 27017:27017 -n crossplane-system &
          #kcl run main.k -D kind=Ars --path_selector items | kubectl apply -f -
          #kubectl apply -k ~/projects/playground/k8s/kustomize

          # kubectl create configmap k6-load-test --from-file=tss.js
          # ^helm repo update
          let cmds = [
            # ["kompose" "convert" "--file" "~/a.yaml" "-n" "dbs" "--stdout" "|" "kubectl" "apply" "-f" "-"] # (see note)
            # ["kubectl" "apply" "-f" "b.yaml"]
            # ["kubectl" "get" "pods" "-A"]
            # ["helm", "upgrade", "--install", "kyverno", "kyverno/kyverno", "--namespace kyverno", "--create-namespace", "--wait"]
            # ["^helm repo add kyverno https://kyverno.github.io/kyverno"]
            # ["^helm repo add external-secrets-operator https://charts.external-secrets.io/"]
            # ["helm", "upgrade", "--install", "my-external-secrets", "external-secrets-operator/external-secrets"]
          ]


        }
      }

      # Inject "in" context for closures (Nu doesn’t bind outer vars automatically inside records)
       let ctx = {
         # gcp_project: $gcp_project
         cluster_name: $name
         k8s_version: $k8s_version
         dry_run: $dry_run
         verbose: $verbose
       }

       let action = ($actions | get $cloud)
       # log info $"($action)"
       # Run the chosen closure with our context via 'do -i' (ignore errors) to preserve controlled exceptions
       do $action --env $ctx
       # kubectl cluster-info --context kind-aris
       # kubectl create namespace dbs
       # kompose convert --file ~/projects/playground/manifests/dockers/compose.yaml -n dbs --stdout | kubectl apply -f -
       # # ps -ef | grep port-forward
       # kubectl port-forward service/db 27017:27017 -n dbs &
       log success $"✅ Cluster flow finished."
}
# source setup-local-machine/
# use local-dev/index.nu

# Check if a kind cluster already exists

# def main [] {
#     ls
#     cluster-exists kind
#     # main shit
# }
