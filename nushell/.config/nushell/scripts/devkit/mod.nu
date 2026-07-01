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

# Bring up the full local development environment
export def "devkit up" [
    --name (-n): string              # Cluster name (default: config cluster.name)
    --workers (-w): int = -1         # Worker nodes (default: config cluster.workers)
    --skip-dbs                       # Skip database deployment
    --skip-secrets                   # Skip external-secrets setup
    --skip-tilt                      # Skip starting Tilt
    --flux                           # Bootstrap Flux GitOps
    --dry-run                        # Preview without executing
    --verbose (-v)                   # Verbose output
] {
    let cfg = (devkit-config)
    let name = (if ($name | is-empty) { $cfg.cluster.name } else { $name })
    let workers = (if $workers < 0 { $cfg.cluster.workers } else { $workers })

    if not (check-prerequisites) {
        error "Missing required prerequisites. Please install them first."
        exit 1
    }

    if $dry_run {
        info "[DRY-RUN] Would create Kind cluster and deploy services"
        info $"  Cluster: ($name), Workers: ($workers)"
        info $"  DBs: (not $skip_dbs), Secrets: (not $skip_secrets)"
        return
    }

    let start_time = date now

    # Step 1: Create Kind cluster
    info $"Step 1/5: Creating Kind cluster '($name)' with ($workers) workers..."
    devkit cluster create -n $name -w $workers --ingress -d 1

    # Step 2: Create app namespaces
    info "Step 2/5: Creating app namespaces..."
    create-app-namespaces $cfg

    # Step 3: Setup External Secrets
    if not $skip_secrets {
        info "Step 3/5: Setting up External Secrets..."
        setup-external-secrets $cfg
    } else {
        info "Step 3/5: Skipping External Secrets setup"
    }

    # Step 4: Deploy databases
    if not $skip_dbs {
        info "Step 4/5: Deploying database services..."
        devkit cluster setup --dbs
        wait-for-databases $cfg
    } else {
        info "Step 4/5: Skipping database deployment"
    }

    # Step 5: Bootstrap Flux (optional)
    if $flux {
        info "Step 5/5: Bootstrapping Flux GitOps..."
        devkit cluster setup --flux
    } else {
        info "Step 5/5: Skipping Flux bootstrap (use --flux to enable)"
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

# Tear down the local development environment
export def "devkit down" [
    --name (-n): string              # Cluster name (default: config cluster.name)
    --keep-cluster                   # Keep cluster, only remove resources
    --verbose (-v)                   # Verbose output
] {
    require-bin "kind"

    let cfg = (devkit-config)
    let name = (if ($name | is-empty) { $cfg.cluster.name } else { $name })

    # Stop Tilt first
    info "Stopping Tilt..."
    do { ^tilt down } | complete

    if $keep_cluster {
        info "Removing resources but keeping cluster..."
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
