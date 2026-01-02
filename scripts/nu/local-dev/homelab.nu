#!/usr/bin/env nu

# Homelab orchestrator - automatically manages Kubernetes clusters based on hardware
# Supports: Kind, k3d, Minikube with automatic selection

use ../shared/shared.nu *
use hardware.nu *

# Create a cluster using the best distribution for your hardware
export def create [
    --name: string = "homelab"
    --workers: int           # Override worker count
    --distribution: string   # Force specific distribution (kind, k3d, minikube)
    --minimal (-m)           # Minimal resources mode
    --verbose (-v)
] {
    let hw = detect-hardware
    let rec = recommend-k8s $hw
    let config = get-cluster-config --name $name --minimal=$minimal

    # Allow distribution override
    let dist = if ($distribution | is-not-empty) { $distribution } else { $rec.primary }
    let worker_count = if ($workers | is-not-empty) { $workers } else { $config.workers }

    log info $"Hardware tier: ($hw.resource_tier | str upcase) | RAM: ($hw.ram_gb)GB | Cores: ($hw.cpu_cores)"
    log info $"Selected distribution: ($dist) | Workers: ($worker_count)"

    # Check Docker runtime
    if $hw.docker_runtime == "none" {
        log error "No Docker runtime detected. Run: homelab install-runtime"
        return
    }

    match $dist {
        "kind" => { create-kind-cluster $name $worker_count $config.ingress $verbose }
        "k3d" => { create-k3d-cluster $name $worker_count $config.ingress $verbose }
        "minikube" => { create-minikube-cluster $name $config $verbose }
        _ => {
            log error $"Unknown distribution: ($dist). Supported: kind, k3d, minikube"
        }
    }

    # Post-creation setup
    if ($rec.features | is-not-empty) {
        log info $"Recommended features for your hardware: ($rec.features | str join ', ')"
        log info "Run 'homelab install-features' to install them"
    }
}

# Delete a homelab cluster
export def delete [
    --name: string = "homelab"
    --all (-a)  # Delete all homelab clusters
] {
    let hw = detect-hardware
    let rec = recommend-k8s $hw

    if $all {
        log info "Deleting all local clusters..."
        delete-by-distribution "kind"
        delete-by-distribution "k3d"
        delete-by-distribution "minikube"
        return
    }

    # Try to detect which distribution the cluster belongs to
    if (cluster-exists-kind $name) {
        kind delete cluster --name $name
        log info $"Deleted Kind cluster: ($name)"
    } else if (cluster-exists-k3d $name) {
        k3d cluster delete $name
        log info $"Deleted k3d cluster: ($name)"
    } else if (cluster-exists-minikube $name) {
        minikube delete -p $name
        log info $"Deleted Minikube cluster: ($name)"
    } else {
        log warning $"Cluster '($name)' not found in any distribution"
    }
}

# List all homelab clusters
export def list [] {
    print $"(ansi cyan)═══ Local Kubernetes Clusters ═══(ansi reset)\n"

    # Kind clusters
    let kind_clusters = if (which kind | is-not-empty) {
        kind get clusters 2>/dev/null | lines | where { $in != "" }
    } else { [] }

    if ($kind_clusters | is-not-empty) {
        print $"(ansi yellow)Kind:(ansi reset)"
        for c in $kind_clusters {
            print $"  • ($c)"
        }
    }

    # k3d clusters
    let k3d_clusters = if (which k3d | is-not-empty) {
        try { k3d cluster list -o json | from json | get name } catch { [] }
    } else { [] }

    if ($k3d_clusters | is-not-empty) {
        print $"(ansi yellow)k3d:(ansi reset)"
        for c in $k3d_clusters {
            print $"  • ($c)"
        }
    }

    # Minikube profiles
    let minikube_clusters = if (which minikube | is-not-empty) {
        try { minikube profile list -o json | from json | get valid | get Name } catch { [] }
    } else { [] }

    if ($minikube_clusters | is-not-empty) {
        print $"(ansi yellow)Minikube:(ansi reset)"
        for c in $minikube_clusters {
            print $"  • ($c)"
        }
    }

    if ($kind_clusters | is-empty) and ($k3d_clusters | is-empty) and ($minikube_clusters | is-empty) {
        print "(ansi dim)No clusters found(ansi reset)"
    }
}

# Show current status
export def status [] {
    show-hardware
    print ""
    list
}

# Install recommended features based on hardware tier
export def install-features [
    --name: string = "homelab"
    --dry-run (-d)
] {
    let hw = detect-hardware
    let rec = recommend-k8s $hw

    log info $"Installing features for ($hw.resource_tier) tier: ($rec.features | str join ', ')"

    for feature in $rec.features {
        if $dry_run {
            log info $"[DRY-RUN] Would install: ($feature)"
            continue
        }

        match $feature {
            "ingress" => { install-ingress-nginx $name }
            "istio" => { install-istio "default" }
            "istio-ambient" => { install-istio "ambient" }
            "monitoring" => { install-monitoring }
            "logging" => { install-logging }
            "metrics-server" => { install-metrics-server }
            "crossplane" => { install-crossplane }
            "vcluster" => { log info "vcluster available via: vcluster create <name>" }
            _ => { log warning $"Unknown feature: ($feature)" }
        }
    }
}

# Bootstrap entire homelab setup
export def bootstrap [
    --name: string = "homelab"
    --skip-runtime
    --skip-cluster
    --skip-features
] {
    log info "Bootstrapping Mac Homelab..."

    let hw = detect-hardware
    let rec = recommend-k8s $hw

    print $"(ansi cyan)═══ Homelab Bootstrap ═══(ansi reset)\n"
    print $"Hardware: ($hw.cpu_brand)"
    print $"Tier: ($hw.resource_tier | str upcase)"
    print $"Distribution: ($rec.primary)"
    print ""

    if not $skip_runtime {
        if $hw.docker_runtime != $rec.recommended_runtime {
            log info $"Installing recommended runtime: ($rec.recommended_runtime)"
            install-runtime
        } else {
            log info $"Runtime OK: ($hw.docker_runtime)"
        }
    }

    if not $skip_cluster {
        log info $"Creating ($rec.primary) cluster..."
        create --name $name
    }

    if not $skip_features {
        log info "Installing recommended features..."
        install-features --name $name
    }

    log info "Homelab bootstrap complete!"
    status
}

# === Internal Functions ===

def create-kind-cluster [name: string, workers: int, ingress: bool, verbose: bool] {
    if (cluster-exists-kind $name) {
        log info $"Kind cluster '($name)' already exists"
        return
    }

    log info $"Creating Kind cluster: ($name) with ($workers) workers"

    # Use KCL to generate config
    let tmp = (_tmpfile $"kind-config-($env.USER)")
    let kcl_response = (kcl run ~/dotconfig/scripts/kcl/stam/main.k -D workers=($workers) -D ingress=($ingress) -D name=($name) | from yaml)
    let config = $kcl_response | get items.0

    $config | to yaml | save -f $tmp --force

    kind create cluster --name $name --config $tmp

    if $env.LAST_EXIT_CODE != 0 {
        rm -f $tmp
        error make {msg: "Failed to create Kind cluster"}
    }

    kubectl cluster-info --context $"kind-($name)"
    kubectl wait --for=condition=Ready nodes --all --timeout=180s

    rm -f $tmp
    log info $"Kind cluster '($name)' created successfully"
}

def create-k3d-cluster [name: string, workers: int, ingress: bool, verbose: bool] {
    if (cluster-exists-k3d $name) {
        log info $"k3d cluster '($name)' already exists"
        return
    }

    log info $"Creating k3d cluster: ($name) with ($workers) agents"

    mut args = ["cluster", "create", $name, "--agents", ($workers | into string)]

    if $ingress {
        $args = ($args | append ["-p" "80:80@loadbalancer" "-p" "443:443@loadbalancer"])
    }

    k3d ...$args

    if $env.LAST_EXIT_CODE != 0 {
        error make {msg: "Failed to create k3d cluster"}
    }

    kubectl cluster-info
    kubectl wait --for=condition=Ready nodes --all --timeout=180s

    log info $"k3d cluster '($name)' created successfully"
}

def create-minikube-cluster [name: string, config: record, verbose: bool] {
    if (cluster-exists-minikube $name) {
        log info $"Minikube cluster '($name)' already exists"
        return
    }

    log info $"Creating Minikube cluster: ($name)"

    minikube start -p $name --memory ($config.memory) --cpus ($config.cpus)

    if $env.LAST_EXIT_CODE != 0 {
        error make {msg: "Failed to create Minikube cluster"}
    }

    log info $"Minikube cluster '($name)' created successfully"
}

def cluster-exists-kind [name: string] -> bool {
    if (which kind | is-empty) { return false }
    let clusters = (kind get clusters 2>/dev/null | lines)
    $name in $clusters
}

def cluster-exists-k3d [name: string] -> bool {
    if (which k3d | is-empty) { return false }
    try {
        let clusters = (k3d cluster list -o json | from json | get name)
        $name in $clusters
    } catch {
        false
    }
}

def cluster-exists-minikube [name: string] -> bool {
    if (which minikube | is-empty) { return false }
    try {
        let profiles = (minikube profile list -o json | from json | get valid | get Name)
        $name in $profiles
    } catch {
        false
    }
}

def delete-by-distribution [dist: string] {
    match $dist {
        "kind" => {
            if (which kind | is-not-empty) {
                let clusters = (kind get clusters 2>/dev/null | lines | where { $in != "" })
                for c in $clusters { kind delete cluster --name $c }
            }
        }
        "k3d" => {
            if (which k3d | is-not-empty) {
                k3d cluster delete --all
            }
        }
        "minikube" => {
            if (which minikube | is-not-empty) {
                minikube delete --all
            }
        }
    }
}

# === Feature Installers ===

def install-ingress-nginx [name: string] {
    log info "Installing NGINX Ingress Controller..."
    kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/main/deploy/static/provider/kind/deploy.yaml
    kubectl wait --namespace ingress-nginx --for=condition=ready pod --selector=app.kubernetes.io/component=controller --timeout=180s
}

def install-istio [profile: string] {
    log info $"Installing Istio with ($profile) profile..."
    istioctl install --set profile=($profile) -y
}

def install-monitoring [] {
    log info "Installing Prometheus + Grafana stack..."
    helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
    helm repo update
    helm install monitoring prometheus-community/kube-prometheus-stack --namespace monitoring --create-namespace
}

def install-logging [] {
    log info "Installing Loki logging stack..."
    helm repo add grafana https://grafana.github.io/helm-charts
    helm repo update
    helm install loki grafana/loki-stack --namespace logging --create-namespace
}

def install-metrics-server [] {
    log info "Installing metrics-server..."
    kubectl apply -f https://github.com/kubernetes-sigs/metrics-server/releases/latest/download/components.yaml
    # Patch for local clusters (insecure TLS)
    kubectl patch -n kube-system deployment metrics-server --type=json -p '[{"op":"add","path":"/spec/template/spec/containers/0/args/-","value":"--kubelet-insecure-tls"}]'
}

def install-crossplane [] {
    log info "Installing Crossplane..."
    helm repo add crossplane-stable https://charts.crossplane.io/stable
    helm repo update
    helm install crossplane crossplane-stable/crossplane --namespace crossplane-system --create-namespace
}
