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
    print "  devkit cluster flux-ui [-p PORT] [--no-open]         open the Flux Web UI"
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
    --flux                           # Install Flux via the Flux Operator (+ Web UI)
    --flux-repo: string              # Flux repository (default: config flux.repository)
    --flux-owner: string             # GitHub owner/org (default: config flux.owner, else gh user)
    --keep-flux-bootstrap            # Keep `<sync path>/flux-system/` in the repo (skip migration cleanup)
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
        setup-flux $cfg ($flux_repo | default "") ($flux_owner | default "") $keep_flux_bootstrap
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
        error $"GitOps overlay not found: ($gitops_path) \(set [paths.overlays].gitops in devkit.toml\)"
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
        error $"Observability overlay not found: ($obs_path) \(set [paths.overlays].observability in devkit.toml\)"
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

# --- Flux (operator-managed) -------------------------------------------------
#
# devkit installs Flux through the ControlPlane Flux Operator instead of
# `flux bootstrap`: the operator owns the controllers and the flux-system
# GitRepository/Kustomization declaratively via a FluxInstance, so re-running
# against a recreated Kind cluster never pushes a commit (the no-op-commit
# failure of fluxcd/flux2#3467 is gone) and the operator pod serves the Flux
# Web UI (`devkit cluster flux-ui`).

const FLUX_OPERATOR_CHART = "oci://ghcr.io/controlplaneio-fluxcd/charts/flux-operator"

# GitHub token for Flux repo operations: config flux.gh_user when set, else the
# active gh account. Repo/deploy-key writes need an account with WRITE access —
# with multiple gh logins the active one is often the wrong identity.
def flux-token [cfg: record]: nothing -> string {
    let gh_user = ($cfg.flux.gh_user? | default "")
    let token_args = (if ($gh_user | is-empty) { [] } else { ["--user" $gh_user] })
    let r = (do { gh auth token ...$token_args } | complete)
    if $r.exit_code != 0 {
        let who = (if ($gh_user | is-empty) { "" } else { $" for user '($gh_user)'" })
        error $"GitHub CLI not authenticated($who). Run 'gh auth login' first."
        exit 1
    }
    $r.stdout | str trim
}

# Make sure owner/repo, the sync branch and the sync path all exist. Unlike
# `flux bootstrap`, the operator never creates repo content, and a GitRepository
# pointing at a missing branch/path leaves the Kustomization failing forever.
def ensure-flux-repo [owner: string, repo: string, branch: string, sync_path: string, token: string] {
    with-env { GH_TOKEN: $token } {
        if ((do { gh api $"repos/($owner)/($repo)" } | complete).exit_code != 0) {
            info $"Creating GitHub repo ($owner)/($repo) \(private\)..."
            let r = (do { gh repo create $"($owner)/($repo)" --private --add-readme } | complete)
            if $r.exit_code != 0 {
                error $"Failed to create ($owner)/($repo): ($r.stderr | str trim)"
                exit 1
            }
        }

        if ((do { gh api $"repos/($owner)/($repo)/branches/($branch)" } | complete).exit_code != 0) {
            let default_branch = (gh api $"repos/($owner)/($repo)" --jq '.default_branch' | str trim)
            let sha = (gh api $"repos/($owner)/($repo)/git/ref/heads/($default_branch)" --jq '.object.sha' | str trim)
            gh api $"repos/($owner)/($repo)/git/refs" -f $"ref=refs/heads/($branch)" -f $"sha=($sha)" | ignore
            info $"Created branch '($branch)' from '($default_branch)'"
        }

        if ((do { gh api $"repos/($owner)/($repo)/contents/($sync_path)?ref=($branch)" } | complete).exit_code != 0) {
            let content = ("# Flux sync path created by devkit\n" | encode base64)
            (gh api --method PUT $"repos/($owner)/($repo)/contents/($sync_path)/README.md"
                -f "message=devkit: seed flux sync path"
                -f $"content=($content)"
                -f $"branch=($branch)") | ignore
            info $"Seeded sync path ($sync_path)/README.md"
        }
    }
}

# HTTPS basic-auth pull secret (username `git`, password = GitHub token).
def create-flux-token-secret [ns: string, token: string] {
    let r = (do {
        kubectl -n $ns create secret generic flux-system --from-literal=username=git $"--from-literal=password=($token)"
    } | complete)
    if $r.exit_code != 0 {
        error $"Failed to create git pull secret: ($r.stderr | str trim)"
        exit 1
    }
    success "Git pull secret 'flux-system' created \(HTTPS token auth\)"
}

# Ensure the `flux-system` pull secret exists and report the scheme the
# FluxInstance must sync with: "ssh" (read-only deploy key minted per cluster)
# or "token" (HTTPS basic auth). `auth` is config flux.auth:
#   auto  — deploy key, falling back to token when the repo rejects it
#           (orgs commonly disable deploy keys: GitHub answers 422
#           "Deploy keys are disabled for this repository")
#   ssh   — deploy key or bust
#   token — HTTPS only, never touches the repo's keys
def ensure-flux-git-secret [ns: string, owner: string, repo: string, token: string, auth: string]: nothing -> string {
    let existing = (do { kubectl -n $ns get secret flux-system -o jsonpath='{.data}' } | complete)
    if $existing.exit_code == 0 {
        let scheme = (if ($existing.stdout | str contains "identity") { "ssh" } else { "token" })
        info $"Git pull secret 'flux-system' already present \(($scheme) auth\) — reusing"
        return $scheme
    }

    if $auth == "token" {
        create-flux-token-secret $ns $token
        return "token"
    }

    require-bin "flux"
    let ssh_url = $"ssh://git@github.com/($owner)/($repo)"
    let key_out = (do {
        flux create secret git flux-system --namespace $ns --url $ssh_url --ssh-key-algorithm ecdsa --ssh-ecdsa-curve p384
    } | complete)
    if $key_out.exit_code != 0 {
        error $"Failed to create git secret: ($key_out.stderr)"
        exit 1
    }

    # Read the public key back from the secret — parsing it out of the CLI
    # output silently yields garbage when the format changes.
    let pub_key = (kubectl -n $ns get secret flux-system -o jsonpath='{.data.identity\.pub}'
        | decode base64 | decode | str trim)

    let ctx = (kubectl config current-context | str trim)
    let title = $"flux-system-($ctx)-(date now | format date '%Y%m%d%H%M%S')"
    let reg = (do {
        with-env { GH_TOKEN: $token } {
            gh api $"repos/($owner)/($repo)/keys" -f $"title=($title)" -f $"key=($pub_key)" -F read_only=true
        }
    } | complete)

    if $reg.exit_code == 0 {
        success $"Deploy key '($title)' registered on ($owner)/($repo) \(read-only\)"
        return "ssh"
    }

    let why = ($reg.stderr + $reg.stdout | str trim | lines | last)
    if $auth == "ssh" {
        error $"($owner)/($repo) rejected the deploy key: ($why)"
        do { kubectl -n $ns delete secret flux-system } | complete
        exit 1
    }

    warn $"($owner)/($repo) rejected the deploy key — falling back to HTTPS token auth: ($why)"
    do { kubectl -n $ns delete secret flux-system } | complete
    create-flux-token-secret $ns $token
    "token"
}

# Drop `<sync_path>/flux-system/` (gotk-components.yaml, gotk-sync.yaml,
# kustomization.yaml) from the repo in a single commit. Mandatory when taking
# over a cluster bootstrapped with `flux bootstrap`: those manifests declare the
# same controllers and the same GitRepository/Kustomization the operator now
# owns, so leaving them in the sync path makes the two fight over every object.
# No-op when the directory is absent.
def prune-flux-bootstrap [owner: string, repo: string, branch: string, sync_path: string, token: string] {
    let dir = $"($sync_path)/flux-system"

    with-env { GH_TOKEN: $token } {
        if ((do { gh api $"repos/($owner)/($repo)/contents/($dir)/gotk-components.yaml?ref=($branch)" } | complete).exit_code != 0) {
            return
        }

        info $"Removing bootstrap manifests ($dir)/ — the operator owns Flux now..."
        let head = (gh api $"repos/($owner)/($repo)/git/ref/heads/($branch)" --jq '.object.sha' | str trim)
        let base_tree = (gh api $"repos/($owner)/($repo)/git/commits/($head)" --jq '.tree.sha' | str trim)
        let blobs = (gh api $"repos/($owner)/($repo)/git/trees/($base_tree)?recursive=1" --jq '.tree[] | select(.type == "blob") | .path'
            | lines
            | where {|p| $p | str starts-with $"($dir)/" })
        if ($blobs | is-empty) { return }

        # A tree entry with a null sha deletes the path.
        let tree = ({
            base_tree: $base_tree
            tree: ($blobs | each {|p| { path: $p, mode: "100644", type: "blob", sha: null } })
        } | to json | gh api --method POST $"repos/($owner)/($repo)/git/trees" --input - --jq '.sha' | str trim)

        let commit = ({
            message: "devkit: remove flux bootstrap manifests (Flux is operator-managed)"
            tree: $tree
            parents: [$head]
        } | to json | gh api --method POST $"repos/($owner)/($repo)/git/commits" --input - --jq '.sha' | str trim)

        gh api --method PATCH $"repos/($owner)/($repo)/git/refs/heads/($branch)" -f $"sha=($commit)" | ignore
        success $"Pruned ($dir)/ from ($owner)/($repo) \(commit ($commit | str substring 0..6)\)"
    }
}

# FluxInstance mirroring the old bootstrap arguments, built from config [flux].
# `scheme` ("ssh" | "token") must match the flux-system pull secret.
def flux-instance [cfg: record, owner: string, repo: string, scheme: string]: nothing -> string {
    let f = $cfg.flux
    let url = (if $scheme == "ssh" {
        $"ssh://git@github.com/($owner)/($repo).git"
    } else {
        $"https://github.com/($owner)/($repo).git"
    })
    {
        apiVersion: "fluxcd.controlplane.io/v1"
        kind: "FluxInstance"
        metadata: { name: "flux", namespace: $f.namespace }
        spec: {
            distribution: { version: $f.version, registry: $f.registry }
            components: $f.components
            cluster: { type: $f.cluster_type, multitenant: false, networkPolicy: true, domain: "cluster.local" }
            sync: {
                kind: "GitRepository"
                url: $url
                ref: $"refs/heads/($f.branch)"
                path: $f.path
                pullSecret: "flux-system"
            }
        }
    } | to yaml
}

# Install the Flux Operator and hand it a FluxInstance. Idempotent: safe to
# re-run on an existing cluster and on a cluster previously bootstrapped with
# `flux bootstrap` (the operator takes over the controllers in place).
def setup-flux [cfg: record, repo_override: string, owner_override: string, keep_bootstrap: bool] {
    require-bin "gh"
    require-bin "helm"

    let f = $cfg.flux
    let ns = $f.namespace
    let repo = (if ($repo_override | is-empty) { $f.repository } else { $repo_override })
    let token = (flux-token $cfg)
    let gh_user = ($f.gh_user? | default "")
    let owner = (if ($owner_override | is-empty) {
        let cfg_owner = ($f.owner? | default "")
        if ($cfg_owner | is-not-empty) {
            $cfg_owner
        } else if ($gh_user | is-not-empty) {
            $gh_user
        } else {
            (with-env { GH_TOKEN: $token } { gh api user --jq '.login' } | str trim)
        }
    } else { $owner_override })

    ensure-flux-repo $owner $repo $f.branch $f.path $token

    do { kubectl create namespace $ns } | complete
    let scheme = (ensure-flux-git-secret $ns $owner $repo $token ($f.auth? | default "auto"))

    info "Installing Flux Operator..."
    let version_args = (if ($f.operator_version? | default "" | is-empty) { [] } else { ["--version" $f.operator_version] })
    let r = (do {
        helm upgrade --install flux-operator $FLUX_OPERATOR_CHART ...$version_args --namespace $ns --create-namespace --wait --timeout 10m
    } | complete)
    if $r.exit_code != 0 {
        error $"Flux Operator install failed \(exit ($r.exit_code)\)"
        print ($r.stdout + $r.stderr)
        exit 1
    }

    info $"Applying FluxInstance \(distribution ($f.version), sync ($owner)/($repo)@($f.branch):($f.path)\)..."
    (flux-instance $cfg $owner $repo $scheme) | kubectl apply -f -

    let w = (do { kubectl -n $ns wait fluxinstance/flux --for=condition=Ready --timeout=300s } | complete)
    if $w.exit_code != 0 {
        warn $"FluxInstance not ready yet — inspect with: kubectl -n ($ns) describe fluxinstance flux"
        info $"Flux Web UI: devkit cluster flux-ui"
        return
    }
    success $"Flux syncing ($owner)/($repo) @ ($f.path) \(operator-managed\)"

    # Only once the operator actually owns the objects: pruning earlier lets the
    # still-live bootstrap Kustomization garbage-collect them.
    if $keep_bootstrap {
        let dir = $"($f.path)/flux-system"
        if ((do { with-env { GH_TOKEN: $token } { gh api $"repos/($owner)/($repo)/contents/($dir)/gotk-components.yaml?ref=($f.branch)" } } | complete).exit_code == 0) {
            warn $"($dir)/ still in the repo — bootstrap manifests and the operator will fight over Flux. Re-run without --keep-flux-bootstrap to prune them."
        }
    } else {
        prune-flux-bootstrap $owner $repo $f.branch $f.path $token
    }

    info $"Flux Web UI: devkit cluster flux-ui"
}

# Port-forward the Flux Operator service and open the built-in Flux Web UI (blocks).
export def "devkit cluster flux-ui" [
    --port (-p): int = -1  # Local port (default: config flux.ui_port)
    --no-open              # Do not launch a browser
] {
    require-bin "kubectl"

    let cfg = (resolve-config)
    let ns = $cfg.flux.namespace
    let port = (if $port < 0 { $cfg.flux.ui_port } else { $port })

    if ((do { kubectl -n $ns get deploy flux-operator } | complete).exit_code != 0) {
        error $"Flux Operator not found in namespace '($ns)'. Run 'devkit cluster setup --flux' first."
        exit 1
    }

    info $"Flux UI: http://localhost:($port)  \(Ctrl-C to stop\)"
    if (not $no_open) and (is-macos) { do { ^open $"http://localhost:($port)" } | complete }
    kubectl -n $ns port-forward svc/flux-operator $"($port):9080"
}

