#!/usr/bin/env nu

# Kubernetes Cluster Management
# Kind cluster creation, management, and post-setup.
# All repo-specific paths/namespaces/defaults come from resolve-config.

use common.nu *
use config.nu *

# Substitute {target} in an overlay path template.
export def overlay-path [template: string, target: string]: nothing -> string {
    $template | str replace --all "{target}" $target
}

# Kind cluster lifecycle + k8s deploys. Run a subcommand, or `help devkit cluster <cmd>`.
export def "devkit cluster" [] {
    print "devkit cluster — Kind cluster lifecycle + k8s deploys"
    print ""
    print "  devkit cluster create [-n NAME] [-w N] [-d N] [-i]   create Kind cluster"
    print "  devkit cluster delete [NAME]                         delete a cluster"
    print "  devkit cluster list                                  list Kind clusters"
    print "  devkit cluster status [-n NAME]                      context + node status"
    print "  devkit cluster setup [--dbs --istio --flux --external-secrets]  post-create infra setup"
    print "  devkit cluster deps                                  install [[deps]] from devkit.toml"
    print "  devkit cluster migrate [-p PORT -u USER ...]         run DB migrations"
    print "  devkit cluster gitops [-e TARGET] [--dry-run]        apply GitOps overlay"
    print "  devkit cluster observability [-e TARGET] [--dry-run] deploy observability stack"
}

# Create a local Kind cluster using KCL configuration
export def "devkit cluster create" [
    --name (-n): string              # Cluster name (default: config cluster.name)
    --workers (-w): int = -1         # Worker nodes (default: config cluster.workers)
    --db-workers (-d): int = -1      # Database-dedicated tainted workers (default: config cluster.db_workers)
    --ingress (-i)                   # Force-enable ingress (ports 80, 443)
    --verbose (-v)                   # Verbose output
] {
    require-bin "kind"
    require-bin "kubectl"
    require-bin "kcl"

    let cfg = (resolve-config)
    let name = (if ($name | is-empty) { $cfg.cluster.name } else { $name })
    let workers = (if $workers < 0 { $cfg.cluster.workers } else { $workers })
    let db_workers = (if $db_workers < 0 { $cfg.cluster.db_workers } else { $db_workers })
    let ingress = ($ingress or $cfg.cluster.ingress)
    let kcl_package = $cfg.cluster.kcl_package
    let kcl_tag = $cfg.cluster.kcl_tag
    # Extra KCL top-level arguments from config (cluster.kcl_args), e.g. oidc_bucket.
    # Flag-driven values below win; empty values are omitted entirely.
    let kcl_args = (
        ($cfg.cluster.kcl_args? | default {})
        | merge { name: $name, workers: $workers, db_workers: $db_workers, ingress: $ingress }
        | transpose key value
        | where {|r| ($r.value | describe) != "nothing" and ($r.value | into string | is-not-empty) }
        | each {|r| ["-D" $"($r.key)=($r.value)"] }
        | flatten
    )

    if (cluster-exists $name) {
        info $"Kind cluster '($name)' already exists - skipping creation"
        # Still ensure kubeconfig has the context and it is current, so
        # follow-up commands (deps, setup) target this cluster.
        kind export kubeconfig --name $name
        return
    }

    info $"Creating Kind cluster: ($name) [workers: ($workers), db-workers: ($db_workers), ingress: ($ingress)]"

    # Generate cluster config using KCL
    let tmp = (tmpfile $"kind-config-($name)")

    let config = (kcl run $kcl_package --tag $kcl_tag ...$kcl_args | lines | skip while {|l| not (($l | str starts-with "kind:") or ($l | str starts-with "apiVersion:"))} | str join "\n" | from yaml)
    $config | to yaml | save -f $tmp --force

    # Capture kind's status before anything else runs: `rm` below would
    # otherwise overwrite $env.LAST_EXIT_CODE and the check would always pass.
    kind create cluster --name $name --config $tmp
    let create_status = ($env.LAST_EXIT_CODE? | default 0)

    rm -f $tmp

    if $create_status != 0 {
        error $"Failed to create cluster '($name)' (kind exit ($create_status))"
        exit 1
    }

    # Wait for cluster to be ready
    kubectl cluster-info --context $"kind-($name)"
    kubectl wait --for=condition=Ready nodes --all --timeout=180s
    kubectl -n kube-system rollout status deploy/coredns --timeout=180s

    success $"Kind cluster '($name)' created successfully"

    if $ingress {
      log info "Ingress enabled (configure Istio/gateway separately if needed)"
    }
}

# Delete a Kind cluster
export def "devkit cluster delete" [
    name?: string  # Cluster name (defaults to config cluster.name)
] {
    require-bin "kind"

    let cfg = (resolve-config)
    let cluster_name = ($name | default $cfg.cluster.name)

    if not (cluster-exists $cluster_name) {
        warn $"Cluster '($cluster_name)' does not exist"
        return
    }

    info $"Deleting Kind cluster: ($cluster_name)"
    kind delete cluster --name $cluster_name
    success $"Cluster '($cluster_name)' deleted"
}

# Install cluster dependencies declared in config `[[deps]]`.
# Helm rows: { name, repo, chart?, version?, namespace?, timeout?, values?, set?, wave? } —
# chart defaults to name, namespace to "default", timeout to "10m"; omit repo
# to use chart as a full reference (e.g. oci://...). `values` is a values file
# path or list of paths (helm -f, later files win), resolved against $PWD then
# the devkit.toml directory; a missing file is a hard error. `set` is a
# key=value string or list of them (helm --set, wins over values files).
# Manifest rows: { name, manifest, wave? } are applied with kubectl
# --server-side (URL or repo-relative path; SSA handles large CRDs).
# Deps in the same wave (default 0) install in PARALLEL; waves run in ascending
# order and a wave only starts after the previous one fully succeeded — give a
# dep a higher wave when it needs another dep's CRDs/webhooks/controllers.
# Command output is captured per dep and only shown on failure. Idempotent.
export def "devkit cluster deps" [] {
    require-bin "kubectl"
    require-cluster-connectivity

    let cfg = (resolve-config)
    let deps = ($cfg.deps? | default [])
    if ($deps | is-empty) {
        info "No [[deps]] declared in devkit.toml — nothing to install"
        return
    }

    # Values paths resolve against the devkit.toml directory so `devkit cluster
    # deps` behaves the same from any subdirectory of the repo.
    let cfg_path = (find-config-file)
    let cfg_dir = (if ($cfg_path | is-empty) { $env.PWD } else { $cfg_path | path dirname })

    # Serial prep: resolve each row into a runnable plan, validate values files,
    # and run every `helm repo add` up front — helm's repo config/cache is not
    # safe to mutate concurrently, and this fails fast before touching the cluster.
    let plans = ($deps | each {|dep|
        let name = $dep.name
        let wave = ($dep.wave? | default 0)
        if ($dep.manifest? | is-not-empty) {
            {
                name: $name
                wave: $wave
                desc: $"manifest ($dep.manifest)"
                done: "applied"
                cmd: "kubectl"
                args: ["apply" "--server-side" "-f" $dep.manifest]
            }
        } else {
            require-bin "helm"
            let chart = ($dep.chart? | default $name)
            let ns = ($dep.namespace? | default "default")
            let timeout = ($dep.timeout? | default "10m")
            let version_args = (if ($dep.version? | is-empty) { [] } else { ["--version" $dep.version] })
            let values = ($dep.values? | default [] | if ($in | describe | str starts-with "list") { $in } else { [$in] })
            let values_args = ($values | each {|v|
                let expanded = ($v | path expand)
                let resolved = (if ($expanded | path exists) { $expanded } else { $cfg_dir | path join $v })
                if not ($resolved | path exists) {
                    error make { msg: $"values file not found for dep ($name): '($v)' \(tried ($expanded) and ($resolved)\)" }
                }
                ["-f" $resolved]
            } | flatten)
            let set_args = ($dep.set? | default [] | if ($in | describe | str starts-with "list") { $in } else { [$in] } | each {|s| ["--set" $s] } | flatten)
            let ref = (if ($dep.repo? | is-not-empty) {
                helm repo add $name $dep.repo --force-update
                $"($name)/($chart)"
            } else { $chart })
            let values_note = (if ($values | is-empty) { "" } else { $", values ($values | str join ', ')" })
            {
                name: $name
                wave: $wave
                desc: $"chart ($ref), namespace ($ns)($values_note)"
                done: "installed"
                cmd: "helm"
                args: (["upgrade" "--install" $name $ref] ++ $version_args ++ $values_args ++ $set_args ++ ["-n" $ns "--create-namespace" "--wait" "--timeout" $timeout])
            }
        }
    })

    # Waves run sequentially; deps within a wave install in parallel.
    let waves = ($plans | get wave | uniq | sort)
    for w in $waves {
        let batch = ($plans | where wave == $w)
        if ($waves | length) > 1 {
            info $"Wave ($w): ($batch | get name | str join ', ')"
        }
        let failed = ($batch | par-each --keep-order {|p|
            info $"Installing dep ($p.name) — ($p.desc)"
            let r = (do { ^$p.cmd ...$p.args } | complete)
            if $r.exit_code == 0 {
                success $"Dep ($p.name) ($p.done)"
            } else {
                error $"Dep ($p.name) failed \(exit ($r.exit_code)\)"
                print ($r.stdout + $r.stderr)
            }
            { name: $p.name, exit_code: $r.exit_code }
        } | where exit_code != 0)
        if ($failed | is-not-empty) {
            error $"Failed deps: ($failed | get name | str join ', ')"
            exit 1
        }
    }
}

# List all Kind clusters
export def "devkit cluster list" [] {
    require-bin "kind"

    let clusters = (kind get clusters | lines | where {|it| $it | is-not-empty})

    if ($clusters | is-empty) {
        info "No Kind clusters found"
        return []
    }

    info $"Found ($clusters | length) Kind cluster\(s):"
    $clusters | each {|c| print $"  - ($c)"}
    $clusters
}

# Get cluster status and context info
export def "devkit cluster status" [
    --name (-n): string  # Specific cluster name
] {
    require-bin "kubectl"

    let result = require-cluster-connectivity

    print ""
    print $"Context: ($result.context)"
    print $"Nodes: ($result.nodes | length)"
    $result.nodes | each {|node| print $"  - ($node)"}

    # Get namespace summary
    let namespaces = (kubectl get ns -o jsonpath='{.items[*].metadata.name}' | split row ' ')
    print ""
    print $"Namespaces: ($namespaces | length)"
}

# Post-cluster setup - deploy common infrastructure
export def "devkit cluster setup" [
    --flux                           # Bootstrap Flux GitOps
    --flux-repo: string              # Flux repository (default: config flux.repository)
    --flux-owner: string             # GitHub owner/org (default: config flux.owner, else gh user)
    --istio                          # Install Istio
    --dbs                            # Deploy database services from compose
    --external-secrets               # Install ESO + GCP credentials + ClusterSecretStore
] {
    require-bin "kubectl"
    require-cluster-connectivity

    let cfg = (resolve-config)

    if $dbs {
        info "Deploying database services..."
        require-bin "kompose"

        let compose_file = $cfg.paths.compose_file
        let dbs_ns = $cfg.namespaces.dbs
        if ($compose_file | path exists) {
            do { kubectl create namespace $dbs_ns } | complete
            let manifests = (kompose convert --file $compose_file --namespace $dbs_ns --stdout)
            # Patch manifests for node placement and Istio protocol detection.
            # Port names must use tcp-* prefix so the client-side Istio sidecar
            # treats non-HTTP traffic as raw TCP passthrough.
            let patched = ($manifests
                | split row "---"
                | where {|s| ($s | str trim) != ""}
                | each {|s|
                    let doc = ($s | from yaml)
                    if ($doc.kind? == "Deployment") {
                        $doc | upsert spec.template.spec.tolerations [{
                            key: "dedicated"
                            value: "database"
                            effect: "NoSchedule"
                        }] | upsert spec.template.spec.nodeSelector { dedicated: "database" }
                    } else if ($doc.kind? == "Service") {
                        let svc_name = ($doc.metadata.name? | default "unknown")
                        $doc | upsert spec.ports ($doc.spec.ports | each {|p|
                            $p | upsert name $"tcp-($svc_name)-($p.port)"
                        })
                    } else {
                        $doc
                    } | to yaml
                }
                | str join "---\n")
            $patched | kubectl apply -f -
            success $"Database services deployed to '($dbs_ns)' namespace \(on db-worker nodes\)"
        } else {
            warn $"Compose file not found at ($compose_file)"
        }
    }

    if $istio {
        info "Installing Istio..."
        require-bin "istioctl"
        istioctl install --set profile=ambient --skip-confirmation
        success "Istio installed"
    }

    if $external_secrets {
        setup-external-secrets $cfg
    }

    if $flux {
        info "Bootstrapping Flux..."
        require-bin "flux"
        require-bin "gh"

        let flux_repo = (if ($flux_repo | is-empty) { $cfg.flux.repository } else { $flux_repo })

        # Token account: config flux.gh_user if set, else the active gh account.
        # Bootstrap pushes commits, so the account needs WRITE on the repo —
        # with multiple gh logins the active one is often the wrong identity.
        let gh_user = ($cfg.flux.gh_user? | default "")
        let token_args = (if ($gh_user | is-empty) { [] } else { ["--user" $gh_user] })
        let token_result = (do { gh auth token ...$token_args } | complete)
        if $token_result.exit_code != 0 {
            let who = (if ($gh_user | is-empty) { "" } else { $" for user '($gh_user)'" })
            error $"GitHub CLI not authenticated($who). Run 'gh auth login' first."
            exit 1
        }

        let owner = (if ($flux_owner | is-empty) {
            let cfg_owner = ($cfg.flux.owner? | default "")
            if ($cfg_owner | is-not-empty) { $cfg_owner } else if ($gh_user | is-not-empty) { $gh_user } else { (gh api user --jq '.login' | str trim) }
        } else { $flux_owner })
        let token = ($token_result.stdout | str trim)
        let branch = $cfg.flux.branch
        let sync_path = $cfg.flux.path

        # Re-running `flux bootstrap` against a repo that already carries
        # identical manifests fails on the no-op commit (fluxcd/flux2#3467) —
        # exactly what happens every time a Kind cluster is recreated. If the
        # repo is already bootstrapped, re-attach this cluster instead:
        # install controllers, mint a fresh read-only deploy key, and recreate
        # the sync objects. No commits are pushed.
        let bootstrapped = ((do {
            with-env { GH_TOKEN: $token } {
                gh api $"repos/($owner)/($flux_repo)/contents/($sync_path)/flux-system/gotk-components.yaml?ref=($branch)"
            }
        } | complete).exit_code) == 0

        if $bootstrapped {
            info $"Repo ($owner)/($flux_repo) already bootstrapped — re-attaching cluster \(no commits\)..."
            flux install

            let ssh_url = $"ssh://git@github.com/($owner)/($flux_repo)"
            let key_out = (flux create secret git flux-system --url $ssh_url --ssh-key-algorithm ecdsa --ssh-ecdsa-curve p384 | complete)
            if $key_out.exit_code != 0 {
                error $"Failed to create git secret: ($key_out.stderr)"
                exit 1
            }
            let pub_key = ($key_out.stderr + $key_out.stdout
                | lines
                | where {|l| $l | str contains "ecdsa-sha2"}
                | first
                | str replace --regex '.*(ecdsa-sha2\S+\s+\S+).*' '$1')

            let ctx = (kubectl config current-context | str trim)
            let title = $"flux-system-($ctx)-(date now | format date '%Y%m%d%H%M%S')"
            with-env { GH_TOKEN: $token } {
                gh api $"repos/($owner)/($flux_repo)/keys" -f $"title=($title)" -f $"key=($pub_key)" -F read_only=true | ignore
            }

            flux create source git flux-system --url $ssh_url --branch $branch --secret-ref flux-system --interval 1m
            flux create kustomization flux-system --source GitRepository/flux-system --path $"./($sync_path)" --prune true --interval 10m
            success $"Flux re-attached to ($owner)/($flux_repo) @ ($sync_path)"
        } else {
            let extra_args = (if $cfg.flux.personal { ["--personal"] } else { [] })

            with-env { GITHUB_TOKEN: $token } {
                flux bootstrap github --owner $owner --repository $flux_repo --branch $branch --path $sync_path ...$extra_args
            }
            success "Flux bootstrapped"
        }
    }
}

# Full external-secrets support: install the External Secrets Operator (unless a
# [[deps]] row named "external-secrets" already owns it), create the GCP
# service-account credentials secret, and apply a ClusterSecretStore so app
# ExternalSecrets can resolve from GCP Secret Manager. Idempotent.
def setup-external-secrets [cfg: record] {
    let es = $cfg.external_secrets
    let ns = $cfg.namespaces.external_secrets

    # Operator. Skip when declared in [[deps]] — `devkit cluster deps` owns it then.
    let in_deps = ($cfg.deps? | default [] | any {|d| $d.name == "external-secrets"})
    if $in_deps {
        info "external-secrets declared in [[deps]] — skipping operator install"
    } else {
        require-bin "helm"
        info "Installing External Secrets Operator..."
        helm repo add external-secrets https://charts.external-secrets.io --force-update
        let version_args = (if ($es.chart_version? | default "" | is-empty) { [] } else { ["--version" $es.chart_version] })
        let r = (do {
            helm upgrade --install external-secrets external-secrets/external-secrets ...$version_args -n $ns --create-namespace --set installCRDs=true --wait --timeout 10m
        } | complete)
        if $r.exit_code != 0 {
            error $"External Secrets Operator install failed \(exit ($r.exit_code)\)"
            print ($r.stdout + $r.stderr)
            exit 1
        }
        success "External Secrets Operator installed"
    }

    # GCP service-account credentials secret
    let creds_path = ($es.gcp_credentials | path expand)
    if not ($creds_path | path exists) {
        warn $"GCP credentials not found at ($creds_path)"
        warn "Skipping credentials secret and ClusterSecretStore — ExternalSecrets will not sync"
        return
    }
    do { kubectl create namespace $ns } | complete
    # create --dry-run | apply = idempotent, and updates the secret when the key file changed
    let sec = (do {
        kubectl create secret generic $es.secret_name -n $ns --from-file=credentials=($creds_path) --dry-run=client -o yaml | kubectl apply -f -
    } | complete)
    if $sec.exit_code != 0 {
        error $"Failed to create secret ($es.secret_name): ($sec.stderr)"
        exit 1
    }
    success $"Secret ($ns)/($es.secret_name) configured from ($creds_path)"

    # ClusterSecretStore. Project id from config, else from the creds JSON itself.
    let project = (if ($es.project_id? | default "" | is-not-empty) {
        $es.project_id
    } else {
        open $creds_path | get -o project_id | default ""
    })
    if ($project | is-empty) {
        warn "No GCP project id (set external_secrets.project_id) — skipping ClusterSecretStore"
        return
    }
    let store_yaml = ({
        apiVersion: "external-secrets.io/v1"
        kind: "ClusterSecretStore"
        metadata: { name: $es.store_name }
        spec: {
            provider: {
                gcpsm: {
                    projectID: $project
                    auth: {
                        secretRef: {
                            secretAccessKeySecretRef: {
                                name: $es.secret_name
                                key: "credentials"
                                namespace: $ns
                            }
                        }
                    }
                }
            }
        }
    } | to yaml)
    # ESO's validating webhook can lag a few seconds behind deployment readiness
    # (CA bundle injection), so retry briefly instead of failing the whole up.
    mut result = { exit_code: 1, stdout: "", stderr: "" }
    for _ in 1..5 {
        $result = (do { $store_yaml | kubectl apply -f - } | complete)
        if $result.exit_code == 0 { break }
        sleep 3sec
    }
    if $result.exit_code == 0 {
        success $"ClusterSecretStore '($es.store_name)' → GCP project ($project)"
    } else {
        error $"Failed to apply ClusterSecretStore '($es.store_name)'"
        print ($result.stdout + $result.stderr)
        exit 1
    }
}

# Run migrations against cluster database
export def "devkit cluster migrate" [
    --port (-p): int = -1            # Database port (default: config database.port)
    --user (-u): string              # Database user (default: config database.user)
    --password: string               # Database password (default: config database.password)
    --database (-d): string          # Database name (default: config database.name)
] {
    let cfg = (resolve-config)
    let port = (if $port < 0 { $cfg.database.port } else { $port })
    let user = (if ($user | is-empty) { $cfg.database.user } else { $user })
    let password = (if ($password | is-empty) { $cfg.database.password } else { $password })
    let database = (if ($database | is-empty) { $cfg.database.name } else { $database })
    let mig_cmd = $cfg.database.migration_cmd

    require-bin ($mig_cmd | first)

    let db_url = $"postgres://($user):($password)@localhost:($port)/($database)"
    info $"Running migrations against localhost:($port)/($database)"

    with-env { DATABASE_URL: $db_url } {
        ^($mig_cmd | first) ...($mig_cmd | skip 1)
    }

    success "Migrations complete"
}

# Deploy GitOps resources using Kustomize
export def "devkit cluster gitops" [
    --target (-e): string             # Environment (default: config paths.default_target)
    --dry-run                          # Preview without applying
] {
    require-bin "kubectl"
    require-cluster-connectivity

    let cfg = (resolve-config)
    let target = (if ($target | is-empty) { $cfg.paths.default_target } else { $target })
    let gitops_path = (overlay-path $cfg.paths.overlays.gitops $target)

    if not ($gitops_path | path exists) {
        error $"GitOps overlay not found: ($gitops_path)"
        exit 1
    }

    info $"Deploying GitOps resources for ($target) environment..."

    if $dry_run {
        kubectl apply -k $gitops_path --dry-run=client
    } else {
        kubectl apply -k $gitops_path
        success $"GitOps resources deployed for ($target)"
    }
}

# Deploy observability stack (Prometheus/Grafana)
export def "devkit cluster observability" [
    --target (-e): string             # Environment (default: config paths.default_target)
    --dry-run                          # Preview without applying
] {
    require-bin "kubectl"
    require-cluster-connectivity

    let cfg = (resolve-config)
    let target = (if ($target | is-empty) { $cfg.paths.default_target } else { $target })
    let obs_path = (overlay-path $cfg.paths.overlays.observability $target)
    let monitoring_ns = $cfg.namespaces.monitoring

    if not ($obs_path | path exists) {
        error $"Observability overlay not found: ($obs_path)"
        exit 1
    }

    info $"Deploying observability stack for ($target) environment..."

    if $dry_run {
        kubectl apply -k $obs_path --dry-run=client
    } else {
        # Create monitoring namespace first
        do { kubectl create namespace $monitoring_ns } | complete

        kubectl apply -k $obs_path
        success $"Observability stack deployed for ($target)"

        if $target == "dev" {
            info "Prometheus will be available after Flux reconciles the HelmRelease"
            info $"Check status: flux get helmreleases -n ($monitoring_ns)"
        }
    }
}

