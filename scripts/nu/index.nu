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

# List installed packages
export def "main list-brew-packages" [] {
    if not (command-exists "brew") {
        return []
    }

    let formulas = (brew list --formula | lines | each { |name| {type: "formula", name: $name} })
    let casks = (brew list --cask | lines | each { |name| {type: "cask", name: $name} })

    $formulas | append $casks

    # log info $"path: $(brewfile_path)"
    # if not ($brewfile_path | path exists) {
    #     log error $"❌ Brewfile not found at: ($brewfile_path)"
    #     exit 1
    # }
    # let packages = (open $brewfile_path | lines | where ($it | str starts-with "brew ") | each { |line|
    #     $line | str replace 'brew "' '' | str replace '"' ''
    # })
    # let os = (sys host | get name)
}

export def "main dev down" [
    --cloud: string = "local"
    --cluster-name: string = "dev"
] {
    _validate-provider $cloud
    match $cloud {
      "aws"   => { _require-bin "nu"; _require-bin "aws" }
      # "gcp"   => { _require-bin "nu"; _require-bin "gcloud" }
      # "azure" => { _require-bin "nu"; _require-bin "az" }
      "local" => { _require-bin "kind"; }
      _ => {}
    }
    delete_cluster $cluster_name
}

export def "main dev up" [
    # --cluster: list<string> # do shit
    --cloud: string = "local"          # One of: aws, gcp, local, azure
    --gitops: string = "flux"          # flux | argo | none (no install here; just pass-through for now)
    --gcp-project: string = "playground-447016"
    --cluster-name(-n): string = "kind"
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
    let local = pwd
    print $"local: ($local)"
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
    if $verbose { log info $"Using temp file)" } else { log info $"Not using temp file" }
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
          # create $cluster_name

          let tmp = (_tmpfile $"kind-config-($env.USER)")
          if $verbose { log info $"Using temp file: ($tmp)" }

          let kcl_response = kcl run ~/dotconfig/scripts/kcl/manager/be/kind-cluster.k -D workers=1 -D ingress=true -D name=$name | from yaml
          let config = $kcl_response | get items.0

          print $config
        # #   # Fetch config with error handling
#           let url = "https://raw.githubusercontent.com/yurikrupnik/gitops/main/cluster/cluster.yaml"
#           let cfg = (do { http get $url } catch {|e|
#             error make { msg: $"Failed to download cluster config from ($url): ($e.msg)" }
#           })
#           mut config = {
#                 kind: "Cluster"
#                 apiVersion: "kind.x-k8s.io/v1alpha4"
#                 name: $cluster_name
#                 nodes: [{
#                     role: "control-plane"
#                 }]
#           }

#           if $enable_ingress {
#             $config = $config | merge {
#                 nodes: [{
#                     role: "control-plane"
#                     kubeadmConfigPatches: [
# 'kind: InitConfiguration
# nodeRegistration:
#     kubeletExtraArgs:
#         node-labels: "ingress-ready=true"'
#                     ]
#                     extraPortMappings: [{
#                         containerPort: 80
#                         hostPort: 80
#                         protocol: "TCP"
#                     }, {
#                         containerPort: 443
#                         hostPort: 443
#                         protocol: "TCP"
#                     },
#                     ]
#                 }]
#             }
#         }
#         if $enable_ha {
#             $config = $config | update nodes ($config.nodes | append [
#               { role: "worker" }
#               { role: "worker" }
#             ])
#             log error $"$config: ($config)"
#         }
#         #let s = (kcl run scripts/kcl/manager/main.k -D workers=1 -D ingress=true -D name=ar)
#         #let kcl_result = (kcl run scripts/kcl/manager/main.k -D workers=1 -D ingress=true -D name=ar).con
#         #print $"kcl result: ($s)"
#         #   print $cfg
#         #   print $dry_run
#         if $verbose { log error $"$config Using config file: ($config)" }
#         if $verbose { log error $"$cfg Using cfg file: ($cfg)" }
        log info $"Using config file: ($config)"
          if not $dry_run {
            # $cfg | save -f $tmp
            # $config | to yaml | save -f $tmp --force
            $config | to yaml | save -f $tmp --force
            $config | to yaml | save -f kind.yaml
            mut cmd = ["kind" "create" "cluster" "--name" $cluster_name "--config" $tmp]
            # kind.yaml
            # if $k8s_version != "" {
            #   $cmd = ($cmd | append ["--image" $"kindest/node:($in.k8s_version)"])
            # }
            if $verbose { log info $"Running: ([$cmd] | str join ' ')" }
            # $cmd.0 | complete
            # create $cluster_name #--config $tmp
            # run kind create cluster --name $cluster_name --config $tmp
            if (cluster-exists $cluster_name) {
                log warning $"Kind cluster '($cluster_name)' already exists — skipping creation."
            } else {
                ^kind create cluster --name $cluster_name --config kind.yaml
                if $env.LAST_EXIT_CODE != 0 {
                  error make { msg: "Command failed" }
                }
                ^kubectl wait --for=condition=Ready nodes --all --timeout=180s
                ^kubectl -n kube-system rollout status deploy/coredns --timeout=180s
                ^kubectl cluster-info --context $"kind-($cluster_name)"

                # DB
                ^kubectl create namespace dbs
                ^kompose convert --file ~/projects/playground/manifests/dockers/compose.yaml --namespace dbs --stdout | kubectl apply -f -;
                # let s = kompose convert --file ~/projects/playground/manifests/dockers/compose.yaml --namespace dbs --stdout;
                # print $"s: &(s)"
            }
            # if $env.LAST_EXIT_CODE != 0 {
            #   error make { msg: "Command failed" }
            # }
            # kubectl cluster-info --context $"kind-($cluster_name)"
            # log info $"command: ($cmd)"
            # run-external $cmd.0 ...($cmd | skip 1)

            # ^($cmd.0) $cmd.1..$cmd | complete
            rm -f $tmp
          } else {
            log info $"Would create Kind cluster '($cluster_name)' with downloaded config"
          }
          do --ignore-errors {
              # kubectl create namespace dbs;
              # ^kompose convert --file ~/projects/playground/manifests/dockers/compose.yaml --namespace dbs --stdout | kubectl apply -f -;
          }

          ^helm repo update
          let cmds = [
            # ["kompose" "convert" "--file" "~/a.yaml" "-n" "dbs" "--stdout" "|" "kubectl" "apply" "-f" "-"] # (see note)
            # ["kubectl" "apply" "-f" "b.yaml"]
            # ["kubectl" "get" "pods" "-A"]
            # ["helm", "upgrade", "--install", "kyverno", "kyverno/kyverno", "--namespace kyverno", "--create-namespace", "--wait"]
            ["^helm repo add kyverno https://kyverno.github.io/kyverno"]
            ["^helm repo add external-secrets-operator https://charts.external-secrets.io/"]
            # ["helm", "upgrade", "--install", "my-external-secrets", "external-secrets-operator/external-secrets"]
          ]

          let jobs = ($cmds
            | par-each {|c|
                let res = (^($c.0) ...($c | skip 1)| complete)
                print $"c: ($c)"
                { cmd: ($c | str join ' '), exit: $res.exit_code, out: $res.stdout, err: $res.stderr }
            })
          let cmds = [
            # ["kompose" "convert" "--file" "~/a.yaml" "-n" "dbs" "--stdout" "|" "kubectl" "apply" "-f" "-"] # (see note)
            # ["kubectl" "apply" "-f" "b.yaml"]
            # ["kubectl" "get" "pods" "-A"]
            ["helm", "upgrade", "--install", "kyverno", "kyverno/kyverno", "--namespace kyverno", "--create-namespace"]
            # ["ls"]
            ["helm", "upgrade", "--install", "my-external-secrets", "external-secrets-operator/external-secrets"]
          ]

          let jobs = ($cmds
            | par-each {|c|
                let res = (^($c.0) ...($c | skip 1)| complete)
                print $"c: ($c)"
                { cmd: ($c | str join ' '), exit: $res.exit_code, out: $res.stdout, err: $res.stderr }
            })

          #print $"jobs: ($jobs)"
          #print $"omg is all i want"
          #let aris1 = nu '-c ls  | get size'
          #let kcl_result = kcl run
          #^ls
          #let aris = ls | to json
          #log error $"omg is all i want ($aris)"
          #log error $"omg is all i want aris1 ($aris1)"
          # let data = kcl run ~/dotconfig/scripts/kcl/manager/main.k -D workers=1 -D ingress=true -D name=ar | from yaml
          # let first_item = $data | get items.0
          # print $first_item
          # | par-each {|c|
          #     # For pipelines, execute them as a block:
          #     do --ignore-errors {
          #       let res = (^($c.0) ...($c | skip 1) | complete)
          #       { cmd: ($c | str join ' '), exit: $res.exit_code, out: $res.stdout, err: $res.stderr }
          #       # ^kompose convert --file ~/a.yaml -n dbs --stdout | ^kubectl apply -f -
          #     }
          # }
          # run-par [
          #     ["docker", "ps"]
          #     ["ls"]
          # ]
          # kubectl create namespace dbs
          # kompose convert --file ~/projects/playground/manifests/dockers/compose.yaml --namespace dbs --stdout | kubectl apply -f -
          # kubectl wait --for=condition=Availible --timeout=300s deployment/db --namespace dbs
          # kubectl wait --for=condition=Availible --timeout=300s service/db --namespace dbs
          # kubectl describe service/db --namespace dbs
          # sleep 60sec
          # kubectl port-forward service/db 27017:27017 -n dbs &
        }
      }

      # Inject "in" context for closures (Nu doesn’t bind outer vars automatically inside records)
       let ctx = {
         gcp_project: $gcp_project
         cluster_name: $cluster_name
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
       log success "✅ Cluster flow finished."
}
# source setup-local-machine/
# use local-dev/index.nu

# Check if a kind cluster already exists

# def main [] {
#     ls
#     cluster-exists kind
#     # main shit
# }
