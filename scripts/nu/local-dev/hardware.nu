#!/usr/bin/env nu

# Hardware detection and Kubernetes distribution selector for Mac homelab
# Automatically selects the optimal K8s distribution based on hardware specs

use ../shared/shared.nu *

# Detect Mac hardware specifications
export def detect-hardware [] -> record {
    let cpu_brand = (sysctl -n machdep.cpu.brand_string | str trim)
    let cpu_arch = (uname -m | str trim)
    let is_apple_silicon = ($cpu_arch == "arm64")

    # Get total RAM in GB
    let ram_bytes = (sysctl -n hw.memsize | into int)
    let ram_gb = ($ram_bytes / 1073741824 | math round)

    # Get CPU cores
    let cpu_cores = (sysctl -n hw.ncpu | into int)
    let perf_cores = if $is_apple_silicon {
        try { sysctl -n hw.perflevel0.logicalcpu | into int } catch { $cpu_cores }
    } else {
        $cpu_cores
    }

    # Get available disk space in GB
    let disk_info = (df -g / | lines | skip 1 | first | split row -r '\s+')
    let disk_available_gb = ($disk_info | get 3 | into int)

    # Detect installed Docker runtime
    let docker_runtime = detect-docker-runtime

    {
        cpu_brand: $cpu_brand
        cpu_arch: $cpu_arch
        is_apple_silicon: $is_apple_silicon
        ram_gb: $ram_gb
        cpu_cores: $cpu_cores
        perf_cores: $perf_cores
        disk_available_gb: $disk_available_gb
        docker_runtime: $docker_runtime
        resource_tier: (classify-resources $ram_gb $cpu_cores)
    }
}

# Detect which Docker runtime is installed
def detect-docker-runtime [] -> string {
    # Check in order of preference for Apple Silicon
    if (which orbctl | is-not-empty) {
        return "orbstack"
    }
    if (which colima | is-not-empty) and ((colima status | complete).exit_code == 0) {
        return "colima"
    }
    if (which docker | is-not-empty) {
        # Check if Docker Desktop
        let docker_info = (docker info 2>/dev/null | complete)
        if ($docker_info.exit_code == 0) {
            if ($docker_info.stdout | str contains "orbstack") {
                return "orbstack"
            } else if ($docker_info.stdout | str contains "colima") {
                return "colima"
            } else if ($docker_info.stdout | str contains "Docker Desktop") {
                return "docker-desktop"
            } else if ($docker_info.stdout | str contains "podman") {
                return "podman"
            }
            return "docker"
        }
    }
    "none"
}

# Classify resource tier based on RAM and CPU
def classify-resources [ram_gb: int, cpu_cores: int] -> string {
    if $ram_gb >= 64 and $cpu_cores >= 10 {
        "beast"
    } else if $ram_gb >= 32 and $cpu_cores >= 8 {
        "power"
    } else if $ram_gb >= 16 and $cpu_cores >= 4 {
        "standard"
    } else {
        "minimal"
    }
}

# Kubernetes distribution recommendations
export def recommend-k8s [hardware: record] -> record {
    let tier = $hardware.resource_tier
    let is_arm = $hardware.is_apple_silicon

    let recommendation = match $tier {
        "beast" => {
            primary: "kind"
            secondary: "talos"
            description: "Full multi-cluster setup with Istio, monitoring stack"
            max_nodes: 10
            features: ["istio", "monitoring", "logging", "vcluster", "crossplane"]
            worker_memory: "8g"
            worker_cpus: 4
        }
        "power" => {
            primary: "kind"
            secondary: "k3d"
            description: "Multi-node cluster with service mesh capability"
            max_nodes: 5
            features: ["istio-ambient", "monitoring", "crossplane"]
            worker_memory: "4g"
            worker_cpus: 2
        }
        "standard" => {
            primary: "kind"
            secondary: "k3d"
            description: "Standard development cluster"
            max_nodes: 3
            features: ["ingress", "metrics-server"]
            worker_memory: "2g"
            worker_cpus: 2
        }
        "minimal" => {
            primary: "k3d"
            secondary: "minikube"
            description: "Lightweight single-node cluster"
            max_nodes: 1
            features: ["ingress"]
            worker_memory: "1g"
            worker_cpus: 1
        }
        _ => {
            primary: "k3d"
            secondary: "minikube"
            description: "Default lightweight setup"
            max_nodes: 1
            features: []
            worker_memory: "1g"
            worker_cpus: 1
        }
    }

    # Docker runtime recommendation
    let runtime_rec = if $is_arm {
        if (which orbctl | is-not-empty) { "orbstack" } else { "colima" }
    } else {
        "docker-desktop"
    }

    $recommendation | merge {
        recommended_runtime: $runtime_rec
        current_runtime: $hardware.docker_runtime
        hardware_tier: $tier
    }
}

# Display hardware info in a nice format
export def show-hardware [] {
    let hw = detect-hardware
    let rec = recommend-k8s $hw

    print $"(ansi cyan)═══ Mac Homelab Hardware Detection ═══(ansi reset)"
    print ""
    print $"(ansi yellow)CPU:(ansi reset)          ($hw.cpu_brand)"
    print $"(ansi yellow)Architecture:(ansi reset) ($hw.cpu_arch) (($hw.is_apple_silicon | if $in { 'Apple Silicon' } else { 'Intel' }))"
    print $"(ansi yellow)RAM:(ansi reset)          ($hw.ram_gb) GB"
    print $"(ansi yellow)CPU Cores:(ansi reset)    ($hw.cpu_cores) (($hw.perf_cores) performance)"
    print $"(ansi yellow)Disk Free:(ansi reset)    ($hw.disk_available_gb) GB"
    print $"(ansi yellow)Docker:(ansi reset)       ($hw.docker_runtime)"
    print $"(ansi yellow)Tier:(ansi reset)         ($hw.resource_tier | str upcase)"
    print ""
    print $"(ansi green)═══ Kubernetes Recommendation ═══(ansi reset)"
    print ""
    print $"(ansi yellow)Primary:(ansi reset)      ($rec.primary)"
    print $"(ansi yellow)Fallback:(ansi reset)     ($rec.secondary)"
    print $"(ansi yellow)Max Nodes:(ansi reset)    ($rec.max_nodes)"
    print $"(ansi yellow)Worker Mem:(ansi reset)   ($rec.worker_memory)"
    print $"(ansi yellow)Worker CPUs:(ansi reset)  ($rec.worker_cpus)"
    print $"(ansi yellow)Features:(ansi reset)     ($rec.features | str join ', ')"
    print $"(ansi yellow)Runtime:(ansi reset)      ($rec.recommended_runtime) (current: ($rec.current_runtime))"
    print ""
    print $"(ansi cyan)($rec.description)(ansi reset)"
}

# Install recommended Docker runtime
export def install-runtime [--force (-f)] {
    let hw = detect-hardware
    let rec = recommend-k8s $hw

    if $hw.docker_runtime == $rec.recommended_runtime and not $force {
        log info $"Already using recommended runtime: ($rec.recommended_runtime)"
        return
    }

    match $rec.recommended_runtime {
        "orbstack" => {
            log info "Installing OrbStack (optimal for Apple Silicon)..."
            brew install --cask orbstack
        }
        "colima" => {
            log info "Installing Colima..."
            brew install colima docker
            colima start --cpu ($hw.perf_cores / 2) --memory ($hw.ram_gb / 2)
        }
        "docker-desktop" => {
            log info "Installing Docker Desktop..."
            brew install --cask docker
        }
        _ => {
            log warning $"Unknown runtime: ($rec.recommended_runtime)"
        }
    }
}

# Install recommended Kubernetes distribution
export def install-k8s [] {
    let hw = detect-hardware
    let rec = recommend-k8s $hw

    log info $"Installing ($rec.primary) for ($hw.resource_tier) tier hardware..."

    match $rec.primary {
        "kind" => {
            if (which kind | is-empty) {
                brew install kind
            }
            log info "Kind installed. Use 'homelab create' to create a cluster."
        }
        "k3d" => {
            if (which k3d | is-empty) {
                brew install k3d
            }
            log info "k3d installed. Use 'homelab create' to create a cluster."
        }
        "minikube" => {
            if (which minikube | is-empty) {
                brew install minikube
            }
            log info "Minikube installed. Use 'homelab create' to create a cluster."
        }
        "talos" => {
            if (which talosctl | is-empty) {
                brew install siderolabs/tap/talosctl
            }
            log info "Talos installed. Advanced VM-based setup required."
        }
        _ => {
            log warning $"Unknown distribution: ($rec.primary)"
        }
    }
}

# Get optimal cluster configuration based on hardware
export def get-cluster-config [
    --name: string = "homelab"
    --minimal (-m)  # Force minimal resources
] -> record {
    let hw = detect-hardware
    let rec = recommend-k8s $hw

    let workers = if $minimal { 0 } else {
        [($rec.max_nodes - 1) 0] | math max
    }

    {
        name: $name
        distribution: $rec.primary
        workers: $workers
        ingress: true
        memory: $rec.worker_memory
        cpus: $rec.worker_cpus
        features: $rec.features
        hardware: $hw
        recommendation: $rec
    }
}
