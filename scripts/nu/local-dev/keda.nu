#!/usr/bin/env nu

# KEDA Management Commands
# Event-driven autoscaling for Kubernetes

use ../shared/shared.nu [log]

const KEDA_MANIFESTS = "k8s-manifests/keda"
const KEDA_VERSION = "2.13.0"

# Install KEDA in the cluster
export def "main keda install" [
    --namespace(-n): string = "keda"  # Namespace for KEDA
    --version(-v): string = ""        # KEDA version
] {
    let version = if $version == "" { $KEDA_VERSION } else { $version }

    log info $"Installing KEDA v($version) in namespace ($namespace)..."

    # Add Helm repo
    helm repo add kedacore https://kedacore.github.io/charts
    helm repo update

    # Install KEDA
    helm install keda kedacore/keda --namespace $namespace --create-namespace --version $version

    # Wait for KEDA to be ready
    kubectl wait --for=condition=ready pod -l app=keda-operator -n $namespace --timeout=120s

    log info "KEDA installed successfully!"
}

# Uninstall KEDA
export def "main keda uninstall" [
    --namespace(-n): string = "keda"
] {
    log info "Uninstalling KEDA..."
    helm uninstall keda -n $namespace
    kubectl delete namespace $namespace
    log info "KEDA uninstalled"
}

# Apply KEDA scalers
export def "main keda apply" [
    scaler: string = "all"  # Scaler to apply: all, prometheus, redis, cron, http, cpu-memory
] {
    log info $"Applying KEDA scaler: ($scaler)"

    if $scaler == "all" {
        kubectl apply -f $KEDA_MANIFESTS
    } else {
        let file = $"($KEDA_MANIFESTS)/($scaler)-scaler.yaml"
        if ($file | path exists) {
            kubectl apply -f $file
        } else {
            log error $"Scaler file not found: ($file)"
        }
    }
}

# List ScaledObjects
export def "main keda list" [
    --all-namespaces(-A)  # List across all namespaces
] {
    log info "Listing ScaledObjects..."

    if $all_namespaces {
        kubectl get scaledobjects -A -o wide
    } else {
        kubectl get scaledobjects -o wide
    }
}

# List ScaledJobs
export def "main keda jobs" [
    --all-namespaces(-A)
] {
    log info "Listing ScaledJobs..."

    if $all_namespaces {
        kubectl get scaledjobs -A -o wide
    } else {
        kubectl get scaledjobs -o wide
    }
}

# Get ScaledObject details
export def "main keda describe" [
    name: string  # ScaledObject name
] {
    kubectl describe scaledobject $name
}

# Get scaling status
export def "main keda status" [
    name: string = ""  # ScaledObject name (optional)
] {
    if $name == "" {
        # Show all
        print "=== ScaledObjects ==="
        kubectl get scaledobjects -o custom-columns='NAME:.metadata.name,TARGET:.spec.scaleTargetRef.name,MIN:.spec.minReplicaCount,MAX:.spec.maxReplicaCount,READY:.status.conditions[?(@.type=="Ready")].status'

        print "\n=== HPAs Created by KEDA ==="
        kubectl get hpa -l scaledobject.keda.sh/name

        print "\n=== ScaledJobs ==="
        kubectl get scaledjobs -o custom-columns='NAME:.metadata.name,MIN:.spec.minReplicaCount,MAX:.spec.maxReplicaCount'
    } else {
        kubectl get scaledobject $name -o yaml
    }
}

# View KEDA operator logs
export def "main keda logs" [
    --follow(-f)          # Follow logs
    --tail(-n): int = 100 # Number of lines
] {
    if $follow {
        kubectl logs -n keda -l app=keda-operator -f --tail $tail
    } else {
        kubectl logs -n keda -l app=keda-operator --tail $tail
    }
}

# Scale a deployment using KEDA
export def "main keda scale" [
    deployment: string         # Deployment name
    --min: int = 1             # Min replicas
    --max: int = 10            # Max replicas
    --trigger: string = "cpu"  # Trigger type: cpu, memory, prometheus, redis, cron
    --threshold: string = "70" # Trigger threshold
] {
    log info $"Creating ScaledObject for ($deployment)..."

    let scaler_yaml = $"
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: ($deployment)-scaler
spec:
  scaleTargetRef:
    name: ($deployment)
  pollingInterval: 30
  cooldownPeriod: 300
  minReplicaCount: ($min)
  maxReplicaCount: ($max)
  triggers:
    - type: ($trigger)
      metricType: Utilization
      metadata:
        value: \"($threshold)\"
"

    echo $scaler_yaml | kubectl apply -f -
    log info "ScaledObject created"
}

# Delete a ScaledObject
export def "main keda delete" [
    name: string  # ScaledObject name
] {
    log info $"Deleting ScaledObject: ($name)"
    kubectl delete scaledobject $name
}

# Pause scaling (set replicas to current)
export def "main keda pause" [
    name: string  # ScaledObject name
] {
    log info $"Pausing ScaledObject: ($name)"
    kubectl annotate scaledobject $name autoscaling.keda.sh/paused-replicas=(kubectl get scaledobject $name -o jsonpath='{.status.scaleTargetReplicas}')
}

# Resume scaling
export def "main keda resume" [
    name: string  # ScaledObject name
] {
    log info $"Resuming ScaledObject: ($name)"
    kubectl annotate scaledobject $name autoscaling.keda.sh/paused-replicas-
}

# Test scale-to-zero
export def "main keda test-zero" [
    deployment: string  # Deployment to test
] {
    log info $"Testing scale-to-zero for ($deployment)..."

    # Create test ScaledObject
    let test_yaml = $"
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: ($deployment)-zero-test
spec:
  scaleTargetRef:
    name: ($deployment)
  pollingInterval: 5
  cooldownPeriod: 10
  minReplicaCount: 0
  maxReplicaCount: 1
  triggers:
    - type: cron
      metadata:
        timezone: UTC
        start: 0 0 1 1 *
        end: 0 0 1 1 *
        desiredReplicas: \"0\"
"

    echo $test_yaml | kubectl apply -f -

    log info "Waiting for scale-to-zero..."
    sleep 30sec

    let replicas = kubectl get deployment $deployment -o jsonpath='{.spec.replicas}'
    if $replicas == "0" {
        log info "Scale-to-zero successful!"
    } else {
        log info $"Current replicas: ($replicas)"
    }

    # Cleanup
    kubectl delete scaledobject $"($deployment)-zero-test"
}

# Check KEDA health
export def "main keda health" [] {
    print "=== KEDA Components ==="
    kubectl get pods -n keda -o wide

    print "\n=== KEDA Operator Status ==="
    kubectl get deployment -n keda keda-operator -o jsonpath='{.status.conditions[*].type}: {.status.conditions[*].status}'

    print "\n=== Metrics Server Status ==="
    kubectl get deployment -n keda keda-metrics-apiserver -o jsonpath='{.status.conditions[*].type}: {.status.conditions[*].status}'

    print "\n=== CRDs ==="
    kubectl get crd | grep keda
}

# Main help
def main [] {
    print "KEDA Management Commands"
    print ""
    print "Installation:"
    print "  keda install     - Install KEDA in cluster"
    print "  keda uninstall   - Remove KEDA"
    print ""
    print "ScaledObjects:"
    print "  keda apply       - Apply KEDA scalers"
    print "  keda list        - List ScaledObjects"
    print "  keda describe    - Describe ScaledObject"
    print "  keda status      - Get scaling status"
    print "  keda scale       - Create ScaledObject"
    print "  keda delete      - Delete ScaledObject"
    print "  keda pause       - Pause scaling"
    print "  keda resume      - Resume scaling"
    print ""
    print "ScaledJobs:"
    print "  keda jobs        - List ScaledJobs"
    print ""
    print "Debugging:"
    print "  keda logs        - View KEDA logs"
    print "  keda health      - Check KEDA health"
    print "  keda test-zero   - Test scale-to-zero"
}
