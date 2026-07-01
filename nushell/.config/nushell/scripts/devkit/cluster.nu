#!/usr/bin/env nu

# Kubernetes Cluster Management
# Kind cluster creation, management, and post-setup.
# All repo-specific paths/namespaces/defaults come from devkit-config.

use common.nu *
use config.nu *

# Substitute {target} in an overlay path template.
def overlay-path [template: string, target: string]: nothing -> string {
    $template | str replace --all "{target}" $target
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

    let cfg = (devkit-config)
    let name = (if ($name | is-empty) { $cfg.cluster.name } else { $name })
    let workers = (if $workers < 0 { $cfg.cluster.workers } else { $workers })
    let db_workers = (if $db_workers < 0 { $cfg.cluster.db_workers } else { $db_workers })
    let ingress = ($ingress or $cfg.cluster.ingress)
    let kcl_package = $cfg.cluster.kcl_package

    if (cluster-exists $name) {
        info $"Kind cluster '($name)' already exists - skipping creation"
        return
    }

    info $"Creating Kind cluster: ($name) [workers: ($workers), db-workers: ($db_workers), ingress: ($ingress)]"

    # Generate cluster config using KCL
    let tmp = (tmpfile $"kind-config-($name)")

    let kcl_response = (kcl run $kcl_package -D workers=($workers) -D db_workers=($db_workers) -D ingress=($ingress) -D name=($name) | lines | skip while {|l| not ($l | str starts-with "items:")} | str join "\n" | from yaml)
    let config = $kcl_response | get items.0
    $config | to yaml | save -f $tmp --force

    kind create cluster --name $name --config $tmp

    rm -f $tmp

    if $env.LAST_EXIT_CODE? == 1 {
        error "Failed to create cluster"
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

    let cfg = (devkit-config)
    let cluster_name = ($name | default $cfg.cluster.name)

    if not (cluster-exists $cluster_name) {
        warn $"Cluster '($cluster_name)' does not exist"
        return
    }

    info $"Deleting Kind cluster: ($cluster_name)"
    kind delete cluster --name $cluster_name
    success $"Cluster '($cluster_name)' deleted"
}

# List all Kind clusters
export def "devkit cluster list" [] {
    require-bin "kind"

    let clusters = (kind get clusters | lines | where {|it| $it | is-not-empty})

    if ($clusters | is-empty) {
        info "No Kind clusters found"
        return []
    }

    info $"Found ($clusters | length) Kind cluster(s):"
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
    --istio                          # Install Istio
    --dbs                            # Deploy database services from compose
] {
    require-bin "kubectl"
    require-cluster-connectivity

    let cfg = (devkit-config)

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
            success $"Database services deployed to '($dbs_ns)' namespace (on db-worker nodes)"
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

    if $flux {
        info "Bootstrapping Flux..."
        require-bin "flux"
        require-bin "gh"

        let flux_repo = (if ($flux_repo | is-empty) { $cfg.flux.repository } else { $flux_repo })

        let token_result = (do { gh auth token } | complete)
        if $token_result.exit_code != 0 {
            error "GitHub CLI not authenticated. Run 'gh auth login' first."
            exit 1
        }

        let owner = (gh api user --jq '.login' | str trim)
        let token = ($token_result.stdout | str trim)

        let extra_args = (if $cfg.flux.personal { ["--personal"] } else { [] })

        with-env { GITHUB_TOKEN: $token } {
            flux bootstrap github --owner $owner --repository $flux_repo --branch $cfg.flux.branch --path $cfg.flux.path ...$extra_args
        }
        success "Flux bootstrapped"
    }
}

# Run migrations against cluster database
export def "devkit cluster migrate" [
    --port (-p): int = -1            # Database port (default: config database.port)
    --user (-u): string              # Database user (default: config database.user)
    --password: string               # Database password (default: config database.password)
    --database (-d): string          # Database name (default: config database.name)
] {
    let cfg = (devkit-config)
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

    let cfg = (devkit-config)
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

    let cfg = (devkit-config)
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

# Full local dev environment setup
export def "devkit cluster local-dev" [
    --name (-n): string              # Cluster name (default: config cluster.name)
    --workers (-w): int = -1         # Worker nodes (default: config cluster.workers)
    --skip-cluster                    # Skip cluster creation
    --skip-dbs                        # Skip database deployment
    --skip-observability              # Skip observability stack
] {
    let cfg = (devkit-config)
    let name = (if ($name | is-empty) { $cfg.cluster.name } else { $name })
    let workers = (if $workers < 0 { $cfg.cluster.workers } else { $workers })
    let target = $cfg.paths.default_target

    info "Setting up full local development environment..."

    # Step 1: Create Kind cluster
    if not $skip_cluster {
        devkit cluster create --name $name --workers $workers --ingress
    }

    # Step 2: Deploy databases
    if not $skip_dbs {
        devkit cluster setup --dbs
    }

    # Step 3: Deploy core infrastructure via GitOps
    info "Deploying core infrastructure..."
    let core_path = (overlay-path $cfg.paths.overlays.core $target)
    kubectl apply -k $core_path

    # Step 4: Deploy observability
    if not $skip_observability {
        devkit cluster observability --target $target
    }

    # Step 5: Wait for services
    info "Waiting for services to be ready..."
    kubectl -n $cfg.namespaces.dbs wait --for=condition=Available deployment --all --timeout=300s 2>/dev/null | complete

    success "Local development environment ready!"
    print ""
    print "Available endpoints:"
    $cfg.endpoints | each {|e| print $"  - ($e.label): ($e.url)" }
}

# Teardown local dev environment
export def "devkit cluster teardown" [
    --name (-n): string              # Cluster name (default: config cluster.name)
    --keep-cluster                    # Keep the cluster, only remove resources
] {
    require-bin "kubectl"

    let cfg = (devkit-config)
    let name = (if ($name | is-empty) { $cfg.cluster.name } else { $name })
    let target = $cfg.paths.default_target

    warn "Tearing down local development environment..."

    if $keep_cluster {
        info "Removing resources but keeping cluster..."
        do { kubectl delete -k (overlay-path $cfg.paths.overlays.observability $target) } | complete
        do { kubectl delete -k (overlay-path $cfg.paths.overlays.core $target) } | complete
        do { kubectl delete ns $cfg.namespaces.dbs } | complete
        success "Resources removed, cluster kept"
    } else {
        devkit cluster delete $name
        success "Local development environment torn down"
    }
}
