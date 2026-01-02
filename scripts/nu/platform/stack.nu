#!/usr/bin/env nu

# Platform Stack - Enterprise Homelab Platform
# Components: Istio, FluxCD, Tekton, Prometheus, Chaos Mesh, Gateway API,
#             Argo Rollouts, Argo Workflows, Dapr, Data Lake/Warehouse

use ../shared/shared.nu *
use ../local-dev/hardware.nu *

# Platform component definitions with dependencies
const PLATFORM_COMPONENTS = {
    # Layer 0: Core Infrastructure
    istio: {
        layer: 0
        description: "Service mesh with Gateway API"
        namespace: "istio-system"
        dependencies: []
        helm_repo: { name: "istio", url: "https://istio-release.storage.googleapis.com/charts" }
    }
    gateway-api: {
        layer: 0
        description: "Kubernetes Gateway API CRDs"
        namespace: "default"
        dependencies: []
    }

    # Layer 1: GitOps & CI/CD
    flux: {
        layer: 1
        description: "GitOps continuous delivery"
        namespace: "flux-system"
        dependencies: []
    }
    crossplane: {
        layer: 1
        description: "Cloud resource management and control plane"
        namespace: "crossplane-system"
        dependencies: []
        helm_repo: { name: "crossplane", url: "https://charts.crossplane.io/stable" }
    }
    tekton: {
        layer: 1
        description: "Cloud-native CI/CD pipelines"
        namespace: "tekton-pipelines"
        dependencies: []
    }

    # Layer 2: Progressive Delivery & Workflows
    argo-rollouts: {
        layer: 2
        description: "Progressive delivery with Istio"
        namespace: "argo-rollouts"
        dependencies: ["istio"]
        helm_repo: { name: "argo", url: "https://argoproj.github.io/argo-helm" }
    }
    argo-workflows: {
        layer: 2
        description: "Workflow engine for AI/async jobs"
        namespace: "argo"
        dependencies: []
        helm_repo: { name: "argo", url: "https://argoproj.github.io/argo-helm" }
    }
    flagger: {
        layer: 2
        description: "Progressive delivery with Istio (Flagger)"
        namespace: "istio-system"
        dependencies: ["istio", "prometheus"]
        helm_repo: { name: "flagger", url: "https://flagger.app" }
    }

    # Layer 3: Data & Integration
    dapr: {
        layer: 3
        description: "Distributed application runtime"
        namespace: "dapr-system"
        dependencies: []
        helm_repo: { name: "dapr", url: "https://dapr.github.io/helm-charts" }
    }

    # Layer 4: Observability
    prometheus: {
        layer: 4
        description: "Monitoring and alerting"
        namespace: "monitoring"
        dependencies: []
        helm_repo: { name: "prometheus-community", url: "https://prometheus-community.github.io/helm-charts" }
    }
    grafana: {
        layer: 4
        description: "Visualization and dashboards"
        namespace: "monitoring"
        dependencies: ["prometheus"]
    }
    kiali: {
        layer: 4
        description: "Istio observability"
        namespace: "istio-system"
        dependencies: ["istio", "prometheus"]
        helm_repo: { name: "kiali", url: "https://kiali.org/helm-charts" }
    }
    jaeger: {
        layer: 4
        description: "Distributed tracing"
        namespace: "observability"
        dependencies: ["istio"]
    }

    # Layer 5: Chaos Engineering
    chaos-mesh: {
        layer: 5
        description: "Chaos engineering platform"
        namespace: "chaos-mesh"
        dependencies: []
        helm_repo: { name: "chaos-mesh", url: "https://charts.chaos-mesh.org" }
    }

    # Layer 6: Data Platform
    minio: {
        layer: 6
        description: "S3-compatible object storage (Data Lake)"
        namespace: "data"
        dependencies: []
        helm_repo: { name: "minio", url: "https://charts.min.io" }
    }
    clickhouse: {
        layer: 6
        description: "OLAP data warehouse"
        namespace: "data"
        dependencies: []
        helm_repo: { name: "clickhouse", url: "https://charts.clickhouse.com" }
    }
    nats: {
        layer: 6
        description: "Message queue for Dapr"
        namespace: "data"
        dependencies: []
        helm_repo: { name: "nats", url: "https://nats-io.github.io/k8s/helm/charts" }
    }

    # Layer 7: Feature Flags
    flagsmith: {
        layer: 7
        description: "Feature flag management platform"
        namespace: "flagsmith"
        dependencies: []
        helm_repo: { name: "flagsmith", url: "https://flagsmith.github.io/helm-charts" }
    }
}

# Install a single platform component
export def install-component [
    component: string
    --skip-deps
    --values: string  # Custom values file
] {
    if $component not-in ($PLATFORM_COMPONENTS | columns) {
        log error $"Unknown component: ($component)"
        log info $"Available: ($PLATFORM_COMPONENTS | columns | str join ', ')"
        return
    }

    let comp = ($PLATFORM_COMPONENTS | get $component)

    # Check dependencies
    if not $skip_deps and ($comp.dependencies | is-not-empty) {
        for dep in $comp.dependencies {
            if not (component-installed $dep) {
                log info $"Installing dependency: ($dep)"
                install-component $dep
            }
        }
    }

    log info $"Installing ($component): ($comp.description)"

    match $component {
        "gateway-api" => { install-gateway-api }
        "istio" => { install-istio-full }
        "flux" => { log info "Flux should be bootstrapped via 'main dev up'. Skipping." }
        "crossplane" => { install-crossplane }
        "tekton" => { install-tekton }
        "argo-rollouts" => { install-argo-rollouts }
        "argo-workflows" => { install-argo-workflows }
        "flagger" => { install-flagger }
        "dapr" => { install-dapr }
        "prometheus" => { helm uninstall prometheus-stack -n monitoring }
        "kiali" => { helm uninstall kiali-server -n istio-system }
        "argo-rollouts" => { helm uninstall argo-rollouts -n argo-rollouts }
        "argo-workflows" => { helm uninstall argo-workflows -n argo }
        "minio" => { helm uninstall minio -n data }
        "clickhouse" => { helm uninstall clickhouse -n data }
        "nats" => { helm uninstall nats -n data }
        "flagsmith" => { helm uninstall flagsmith -n flagsmith }
        _ => { log warning $"No uninstaller for: ($component)" }
    }

    log info $"($component) uninstalled"
}

# Show platform status
export def status [] {
    print $"(ansi cyan)═══ Platform Stack Status ═══(ansi reset)\n"

    for comp_name in ($PLATFORM_COMPONENTS | columns) {
        let comp = ($PLATFORM_COMPONENTS | get $comp_name)
        let installed = (component-installed $comp_name)
        let status_icon = if $installed { $"(ansi green)✓(ansi reset)" } else { $"(ansi dim)○(ansi reset)" }
        let status_text = if $installed { "(ansi green)installed(ansi reset)" } else { "(ansi dim)not installed(ansi reset)" }
        print $"($status_icon) ($comp_name | fill -w 15) ($status_text) - ($comp.description)"
    }
}

# Check if component is installed
def component-installed [name: string] -> bool {
    let comp = ($PLATFORM_COMPONENTS | get $name)
    let ns = $comp.namespace

    # Check if namespace exists and has running pods
    let ns_exists = (kubectl get ns $ns --no-headers 2>/dev/null | complete).exit_code == 0
    if not $ns_exists { return false }

    # Component-specific checks
    match $name {
        "istio" => { (kubectl get deploy istiod -n istio-system --no-headers 2>/dev/null | complete).exit_code == 0 }
        "gateway-api" => { (kubectl get crd gateways.gateway.networking.k8s.io 2>/dev/null | complete).exit_code == 0 }
        "flux" => { (kubectl get deploy source-controller -n flux-system --no-headers 2>/dev/null | complete).exit_code == 0 }
        "tekton" => { (kubectl get deploy tekton-pipelines-controller -n tekton-pipelines --no-headers 2>/dev/null | complete).exit_code == 0 }
        "argo-rollouts" => { (kubectl get deploy argo-rollouts -n argo-rollouts --no-headers 2>/dev/null | complete).exit_code == 0 }
        "argo-workflows" => { (kubectl get deploy argo-workflows-server -n argo --no-headers 2>/dev/null | complete).exit_code == 0 }
        "dapr" => { (kubectl get deploy dapr-operator -n dapr-system --no-headers 2>/dev/null | complete).exit_code == 0 }
        "prometheus" => { (kubectl get deploy prometheus-stack-kube-prom-operator -n monitoring --no-headers 2>/dev/null | complete).exit_code == 0 }
        "kiali" => { (kubectl get deploy kiali -n istio-system --no-headers 2>/dev/null | complete).exit_code == 0 }
        "chaos-mesh" => { (kubectl get deploy chaos-controller-manager -n chaos-mesh --no-headers 2>/dev/null | complete).exit_code == 0 }
        "crossplane" => { (kubectl get deploy crossplane -n crossplane-system --no-headers 2>/dev/null | complete).exit_code == 0 }
        "flagger" => { (kubectl get deploy flagger -n istio-system --no-headers 2>/dev/null | complete).exit_code == 0 }
        "flagsmith" => { (kubectl get deploy flagsmith -n flagsmith --no-headers 2>/dev/null | complete).exit_code == 0 }
        "minio" => { (kubectl get deploy minio -n data --no-headers 2>/dev/null | complete).exit_code == 0 }
        "clickhouse" => { (kubectl get sts clickhouse -n data --no-headers 2>/dev/null | complete).exit_code == 0 }
        "nats" => { (kubectl get deploy nats-box -n data --no-headers 2>/dev/null | complete).exit_code == 0 }
        _ => false
    }
}

# === Component Installers ===

def install-gateway-api [] {
    log info "Installing Gateway API CRDs..."
    kubectl apply -f https://github.com/kubernetes-sigs/gateway-api/releases/download/v1.2.0/standard-install.yaml
}

def install-istio-full [] {
    log info "Installing Istio with Gateway API support..."

    # Install Gateway API first
    install-gateway-api

    # Install Istio with ambient mode and Gateway API
    istioctl install --set profile=ambient --set values.pilot.env.PILOT_ENABLE_GATEWAY_API=true -y

    # Wait for Istio
    kubectl wait -n istio-system deployment/istiod --for=condition=Available --timeout=300s

    log info "Istio installed with Gateway API support"
}

def install-tekton [] {
    log info "Installing Tekton Pipelines..."
    kubectl apply --filename https://storage.googleapis.com/tekton-releases/pipeline/latest/release.yaml

    log info "Installing Tekton Triggers..."
    kubectl apply --filename https://storage.googleapis.com/tekton-releases/triggers/latest/release.yaml
    kubectl apply --filename https://storage.googleapis.com/tekton-releases/triggers/latest/interceptors.yaml

    log info "Installing Tekton Dashboard..."
    kubectl apply --filename https://storage.googleapis.com/tekton-releases/dashboard/latest/release.yaml

    kubectl wait -n tekton-pipelines deployment/tekton-pipelines-controller --for=condition=Available --timeout=180s
    log info "Tekton installed"
}

def install-argo-rollouts [] {
    log info "Installing Argo Rollouts with Istio integration..."

    kubectl create namespace argo-rollouts --dry-run=client -o yaml | kubectl apply -f -

    helm repo add argo https://argoproj.github.io/argo-helm
    helm repo update

    # Install with Istio traffic management
    helm upgrade --install argo-rollouts argo/argo-rollouts -n argo-rollouts --set dashboard.enabled=true --set controller.trafficRouterPlugins.istio.enabled=true

    kubectl wait -n argo-rollouts deployment/argo-rollouts --for=condition=Available --timeout=180s
    log info "Argo Rollouts installed"
}

def install-argo-workflows [] {
    log info "Installing Argo Workflows..."

    kubectl create namespace argo --dry-run=client -o yaml | kubectl apply -f -

    helm repo add argo https://argoproj.github.io/argo-helm
    helm repo update

    helm upgrade --install argo-workflows argo/argo-workflows -n argo --set server.extraArgs=["--auth-mode=server"] --set controller.workflowDefaults.spec.serviceAccountName=argo-workflow

    # Create workflow service account
    kubectl create sa argo-workflow -n argo --dry-run=client -o yaml | kubectl apply -f -
    kubectl create rolebinding argo-workflow-binding --clusterrole=admin --serviceaccount=argo:argo-workflow -n argo --dry-run=client -o yaml | kubectl apply -f -

    kubectl wait -n argo deployment/argo-workflows-server --for=condition=Available --timeout=180s
    log info "Argo Workflows installed"
}

def install-dapr [] {
    log info "Installing Dapr..."

    # Use Dapr CLI for installation
    if (which dapr | is-empty) {
        log error "Dapr CLI not found. Install with: brew install dapr/tap/dapr-cli"
        return
    }

    dapr init -k --enable-mtls=true

    kubectl wait -n dapr-system deployment/dapr-operator --for=condition=Available --timeout=180s
    log info "Dapr installed"
}

def install-prometheus-stack [] {
    log info "Installing Prometheus + Grafana stack..."

    kubectl create namespace monitoring --dry-run=client -o yaml | kubectl apply -f -

    helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
    helm repo update

    # Install with Istio scraping enabled
    helm upgrade --install prometheus-stack prometheus-community/kube-prometheus-stack -n monitoring --set prometheus.prometheusSpec.podMonitorSelectorNilUsesHelmValues=false --set prometheus.prometheusSpec.serviceMonitorSelectorNilUsesHelmValues=false

    kubectl wait -n monitoring deployment/prometheus-stack-kube-prom-operator --for=condition=Available --timeout=300s
    log info "Prometheus stack installed"
}

def install-kiali [] {
    log info "Installing Kiali..."

    helm repo add kiali https://kiali.org/helm-charts
    helm repo update

    helm upgrade --install kiali-server kiali/kiali-server -n istio-system --set auth.strategy=anonymous --set external_services.prometheus.url="http://prometheus-stack-kube-prom-prometheus.monitoring:9090"

    kubectl wait -n istio-system deployment/kiali --for=condition=Available --timeout=180s
    log info "Kiali installed"
}

def install-jaeger [] {
    log info "Installing Jaeger..."

    kubectl create namespace observability --dry-run=client -o yaml | kubectl apply -f -

    kubectl apply -f https://github.com/jaegertracing/jaeger-operator/releases/download/v1.62.0/jaeger-operator.yaml -n observability

    kubectl wait -n observability deployment/jaeger-operator --for=condition=Available --timeout=180s
    log info "Jaeger operator installed"
}

def install-chaos-mesh [] {
    log info "Installing Chaos Mesh..."

    kubectl create namespace chaos-mesh --dry-run=client -o yaml | kubectl apply -f -

    helm repo add chaos-mesh https://charts.chaos-mesh.org
    helm repo update

    helm upgrade --install chaos-mesh chaos-mesh/chaos-mesh -n chaos-mesh --set chaosDaemon.runtime=containerd --set chaosDaemon.socketPath=/run/containerd/containerd.sock --set dashboard.securityMode=false

    kubectl wait -n chaos-mesh deployment/chaos-controller-manager --for=condition=Available --timeout=180s
    log info "Chaos Mesh installed"
}

def install-minio [] {
    log info "Installing MinIO (Data Lake)..."

    kubectl create namespace data --dry-run=client -o yaml | kubectl apply -f -

    helm repo add minio https://charts.min.io
    helm repo update

    helm upgrade --install minio minio/minio -n data --set mode=standalone --set rootUser=admin --set rootPassword=minio123 --set persistence.size=10Gi --set consoleService.type=ClusterIP

    kubectl wait -n data deployment/minio --for=condition=Available --timeout=180s
    log info "MinIO installed"
}

def install-clickhouse [] {
    log info "Installing ClickHouse (Data Warehouse)..."

    kubectl create namespace data --dry-run=client -o yaml | kubectl apply -f -

    # Using Altinity operator
    helm repo add altinity https://helm.altinity.com
    helm repo update

    helm upgrade --install clickhouse-operator altinity/altinity-clickhouse-operator -n data

    kubectl wait -n data deployment/clickhouse-operator --for=condition=Available --timeout=180s
    log info "ClickHouse operator installed"
}

def install-nats [] {
    log info "Installing NATS (Message Queue)..."

    kubectl create namespace data --dry-run=client -o yaml | kubectl apply -f -

    helm repo add nats https://nats-io.github.io/k8s/helm/charts
    helm repo update

    helm upgrade --install nats nats/nats -n data --set jetstream.enabled=true

    kubectl wait -n data statefulset/nats --for=jsonpath='{.status.readyReplicas}'=1 --timeout=180s
    log info "NATS installed with JetStream"
}

def install-crossplane [] {
    log info "Installing Crossplane (Cloud Resource Management)..."

    kubectl create namespace crossplane-system --dry-run=client -o yaml | kubectl apply -f -

    helm repo add crossplane https://charts.crossplane.io/stable
    helm repo update

    helm upgrade --install crossplane crossplane/crossplane -n crossplane-system

    kubectl wait -n crossplane-system deployment/crossplane --for=condition=Available --timeout=180s
    log info "Crossplane installed"
}

def install-flagger [] {
    log info "Installing Flagger (Progressive Delivery)..."

    helm repo add flagger https://flagger.app
    helm repo update

    helm upgrade --install flagger flagger/flagger -n istio-system --set meshProvider=istio --set metricsServer=http://prometheus-stack-kube-prom-prometheus.monitoring:9090

    kubectl wait -n istio-system deployment/flagger --for=condition=Available --timeout=180s
    log info "Flagger installed"
}

def install-flagsmith [] {
    log info "Installing Flagsmith (Feature Flags)..."

    kubectl create namespace flagsmith --dry-run=client -o yaml | kubectl apply -f -

    helm repo add flagsmith https://flagsmith.github.io/helm-charts
    helm repo update

    helm upgrade --install flagsmith flagsmith/flagsmith -n flagsmith --set postgresql.enabled=true --set redis.enabled=true

    kubectl wait -n flagsmith deployment/flagsmith --for=condition=Available --timeout=300s
    log info "Flagsmith installed"
}
