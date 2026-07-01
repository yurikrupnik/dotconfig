#!/usr/bin/env nu

# devkit - reusable monorepo dev/ops engine
#
# Install once (see install.nu), then in any monorepo:
#     use devkit *
#     devkit up                 # full local environment
#     devkit cluster create     # just the Kind cluster
#     devkit dev up -d          # docker compose
#     devkit secrets fetch      # pull secrets via vals
#     devkit setup install -a   # install toolchain
#
# Repo-specific facts (paths, namespaces, endpoints, flux repo, ...) are read
# from a `devkit.toml` discovered by walking up from $PWD. See config.nu for the
# full schema and built-in defaults.

export use common.nu *
export use config.nu *
export use cluster.nu *
export use local-dev.nu *
export use secrets.nu *
export use setup.nu *

# ============================================================================
# Prerequisites
# ============================================================================

# Check all required tools are installed
def check-prerequisites []: nothing -> bool {
    let required = ["kind" "kubectl" "tilt" "kcl" "kompose" "istioctl"]
    let optional = ["flux" "gh" "helm"]

    info "Checking prerequisites..."

    mut all_found = true
    for cmd in $required {
        if (command-exists $cmd) {
            success $"  ($cmd)"
        } else {
            error $"  ($cmd) - NOT FOUND (required)"
            $all_found = false
        }
    }

    for cmd in $optional {
        if (command-exists $cmd) {
            success $"  ($cmd)"
        } else {
            warn $"  ($cmd) - not found (optional)"
        }
    }

    $all_found
}

# ============================================================================
# Top-level lifecycle: up / down / status
# ============================================================================

# Overview of the most-used commands. Run a group (e.g. `devkit cluster`) to list
# its subcommands, or `help devkit <cmd>` for details on any command.
export def main [] {
    print "devkit — reusable monorepo dev/ops engine"
    print ""
    print "Daily:"
    print "  devkit up [--istio --core --gitops --observability --flux]  bring up local env (ingress always on)"
    print "  devkit down [--keep-cluster]                                tear it down"
    print "  devkit status                                               cluster + namespace status"
    print "  devkit config [--data | --path]                             view effective config"
    print "  devkit config init [dir] [--force]                          scaffold devkit.toml"
    print ""
    print "Building blocks — run the group name to list its subcommands:"
    print "  devkit cluster    Kind cluster lifecycle + k8s deploys"
    print "  devkit dev        docker compose wrappers"
    print "  devkit secrets    vals / vault secret handling"
    print "  devkit setup      toolchain install, build, check, test"
}

# Bring up the local development environment. Extras are opt-in via flags:
# --istio, --core, --gitops, --observability, --flux. Ingress is always enabled.
export def "devkit up" [
    --name (-n): string              # Cluster name (default: config cluster.name)
    --workers (-w): int = -1         # Worker nodes (default: config cluster.workers)
    --skip-dbs                       # Skip database deployment
    --skip-secrets                   # Skip external-secrets setup
    --skip-tilt                      # Skip starting Tilt
    --istio                          # Install Istio (ambient) before workloads
    --core                           # Apply the core infrastructure overlay
    --gitops                         # Apply the GitOps overlay
    --observability                  # Deploy the observability stack
    --flux                           # Bootstrap Flux GitOps
    --dry-run                        # Preview without executing
    --verbose (-v)                   # Verbose output
] {
    let cfg = (resolve-config)
    let name = (if ($name | is-empty) { $cfg.cluster.name } else { $name })
    let workers = (if $workers < 0 { $cfg.cluster.workers } else { $workers })
    let target = $cfg.paths.default_target

    if not (check-prerequisites) {
        error "Missing required prerequisites. Please install them first."
        exit 1
    }

    if $dry_run {
        info "[DRY-RUN] Would create Kind cluster and deploy services"
        info $"  Cluster: ($name), Workers: ($workers)"
        info $"  DBs: (not $skip_dbs), Secrets: (not $skip_secrets), Istio: ($istio), Core: ($core), GitOps: ($gitops), Observability: ($observability), Flux: ($flux)"
        return
    }

    let start_time = date now

    # Kind cluster (ingress always on)
    info $"Creating Kind cluster '($name)' with ($workers) workers..."
    devkit cluster create -n $name -w $workers --ingress -d 1

    # Service mesh before workloads (db manifests are patched for Istio ambient)
    if $istio {
        info "Installing Istio..."
        devkit cluster setup --istio
    }

    # App namespaces
    info "Creating app namespaces..."
    create-app-namespaces $cfg

    # External Secrets
    if not $skip_secrets {
        info "Setting up External Secrets..."
        setup-external-secrets $cfg
    } else {
        info "Skipping External Secrets setup"
    }

    # Databases
    if not $skip_dbs {
        info "Deploying database services..."
        devkit cluster setup --dbs
        wait-for-databases $cfg
    } else {
        info "Skipping database deployment"
    }

    # Core infrastructure overlay
    if $core {
        let core_path = (overlay-path $cfg.paths.overlays.core $target)
        if ($core_path | path exists) {
            info $"Applying core overlay: ($core_path)"
            kubectl apply -k $core_path
        } else {
            warn $"Core overlay not found: ($core_path)"
        }
    }

    # GitOps overlay
    if $gitops {
        info "Deploying GitOps resources..."
        devkit cluster gitops --target $target
    }

    # Observability stack
    if $observability {
        info "Deploying observability stack..."
        devkit cluster observability --target $target
    }

    # Flux GitOps bootstrap
    if $flux {
        info "Bootstrapping Flux GitOps..."
        devkit cluster setup --flux
    }

    let elapsed = (date now) - $start_time

    success $"Environment '($name)' is ready! (($elapsed | format duration sec))"
    print ""
    print-endpoints $cfg

    # Start Tilt
    if (not $skip_tilt) and $cfg.tilt.enabled {
        print ""
        info "Starting Tilt..."
        ^tilt up
    } else {
        print ""
        print "Next steps:"
        print "  - Run 'tilt up' to start application development"
        print "  - Run 'devkit down' to tear down the environment"
    }
}

# Tear down the local development environment.
export def "devkit down" [
    --name (-n): string              # Cluster name (default: config cluster.name)
    --keep-cluster                   # Keep cluster, only remove deployed resources
    --verbose (-v)                   # Verbose output
] {
    require-bin "kind"

    let cfg = (resolve-config)
    let name = (if ($name | is-empty) { $cfg.cluster.name } else { $name })
    let target = $cfg.paths.default_target

    # Stop Tilt first
    info "Stopping Tilt..."
    do { ^tilt down } | complete

    if $keep_cluster {
        info "Removing resources but keeping cluster..."
        # Delete overlays devkit up may have applied (ignore missing paths)
        for ov in [$cfg.paths.overlays.core $cfg.paths.overlays.gitops $cfg.paths.overlays.observability] {
            let p = (overlay-path $ov $target)
            if ($p | path exists) {
                do { kubectl delete -k $p --ignore-not-found } | complete
            }
        }
        # Delete lifecycle namespaces
        for ns in (lifecycle-namespaces $cfg) {
            do { kubectl delete ns $ns --ignore-not-found } | complete
        }
        success "Resources removed, cluster kept"
        print ""
        print $"Cluster '($name)' is still running. To delete it:"
        print $"  devkit down  # or: kind delete cluster --name ($name)"
    } else {
        info $"Deleting Kind cluster: ($name)"
        devkit cluster delete $name
        success $"Environment '($name)' is down"
    }
}

# Show environment status
export def "devkit status" [
    --cloud (-c): string = "local"   # Cloud provider
] {
    match $cloud {
        "local" => {
            devkit cluster list
            devkit cluster status
        }
        _ => {
            warn $"Status for ($cloud) not yet implemented"
        }
    }
}

# ============================================================================
# Helper functions (config passed in explicitly)
# ============================================================================

# Namespaces the lifecycle owns and may delete on teardown
def lifecycle-namespaces [cfg: record]: nothing -> list<string> {
    let app_ns = (resolve-app-namespaces $cfg)
    ($app_ns
        | append $cfg.namespaces.dbs
        | append $cfg.namespaces.monitoring
        | append $cfg.namespaces.external_secrets
        | uniq)
}

# Resolve app namespaces from config, or derive from apps_dir if empty
def resolve-app-namespaces [cfg: record]: nothing -> list<string> {
    if ($cfg.app_namespaces | is-not-empty) {
        return $cfg.app_namespaces
    }
    let apps_dir = $cfg.paths.apps_dir
    if not ($apps_dir | path exists) {
        return []
    }
    ls $apps_dir | where type == dir | get name | path basename
}

# Create app namespaces
def create-app-namespaces [cfg: record] {
    let namespaces = (resolve-app-namespaces $cfg)

    if ($namespaces | is-empty) {
        warn $"No app namespaces configured and ($cfg.paths.apps_dir)/ not found, skipping"
        return
    }

    for ns in $namespaces {
        info $"  Creating namespace: ($ns)"
        do { kubectl create namespace $ns } | complete
    }

    success $"Created ($namespaces | length) app namespaces: ($namespaces | str join ', ')"
}

# Setup External Secrets with GCP credentials
def setup-external-secrets [cfg: record] {
    let creds_path = ($cfg.external_secrets.gcp_credentials | path expand)
    let es_ns = $cfg.namespaces.external_secrets

    if not ($creds_path | path exists) {
        warn $"GCP credentials not found at ($creds_path)"
        warn "External Secrets will not be able to pull from GCP Secret Manager"
        return
    }

    do { kubectl create namespace $es_ns } | complete

    let result = do {
        kubectl create secret generic $cfg.external_secrets.secret_name -n $es_ns --from-file=credentials=($creds_path)
    } | complete

    if $result.exit_code == 0 {
        success "External Secrets configured with GCP credentials"
    } else if ($result.stderr | str contains "already exists") {
        info "External Secrets credentials already configured"
    } else {
        warn $"Failed to create secret: ($result.stderr)"
    }
}

# Wait for database pods to be ready
def wait-for-databases [cfg: record] {
    info "Waiting for databases to be ready..."

    let result = do {
        kubectl wait --for=condition=Available deployment --all -n $cfg.namespaces.dbs --timeout=($cfg.database.wait_timeout)
    } | complete

    if $result.exit_code == 0 {
        success "All databases are ready"
    } else {
        warn "Some databases may not be ready yet"
    }
}

# Print available endpoints from config
def print-endpoints [cfg: record] {
    print "Available endpoints (after Tilt starts):"
    let width = ($cfg.endpoints | each {|e| ($e.label | str length)} | math max)
    for e in $cfg.endpoints {
        let label = ($e.label | fill -a left -w $width)
        print $"  ($label)  ($e.url)"
    }
}
