# CNCF Platform Stack
# ====================
# This module deploys CNCF projects for a complete cloud-native platform:
#
# Observability (managed via Grafana UI):
# 1. Prometheus (CNCF Graduated) - Metrics collection & alerting
# 2. Grafana - UI Management Layer for all observability data
# 3. Jaeger (CNCF Graduated) - Distributed tracing
# 4. OpenTelemetry Collector (CNCF Incubating) - Unified telemetry
# 5. Loki - Log aggregation (Grafana/CNCF ecosystem)
#
# Platform Tools:
# 6. KEDA (CNCF Graduated) - Event-driven autoscaling
# 7. FluxCD (CNCF Graduated) - GitOps continuous delivery
# 8. Dapr (CNCF Graduated) - Distributed application runtime
#
# Integration:
# - KEDA scales workloads based on Prometheus metrics and Dapr pub/sub
# - FluxCD manages GitOps deployments of all components
# - Dapr sends traces to OpenTelemetry Collector -> Jaeger
# - All metrics flow to Prometheus -> Grafana
#
# Sources:
# - https://www.cncf.io/projects/
# - https://landscape.cncf.io/

import k8s.api.core.v1 as corev1

# Observability components
import .prometheus as prom
import .grafana as graf
import .jaeger as jaeg
import .otel_collector as otel
import .loki as lok

# Platform components
import .keda as keda
import .fluxcd as flux
import .dapr as dapr

# Configuration options - Observability
enable_prometheus: bool = option("prometheus") or True
enable_grafana: bool = option("grafana") or True
enable_jaeger: bool = option("jaeger") or True
enable_otel: bool = option("otel") or True
enable_loki: bool = option("loki") or True

# Configuration options - Platform
enable_keda: bool = option("keda") or True
enable_fluxcd: bool = option("fluxcd") or True
enable_dapr: bool = option("dapr") or True

NAMESPACE = "monitoring"

# Monitoring Namespace
_namespace = corev1.Namespace {
    metadata = {
        name = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "observability"
            "app.kubernetes.io/managed-by" = "kcl"
            "cncf.io/stack" = "platform"
            # Enable Dapr sidecar injection for monitoring namespace
            "dapr.io/enabled" = "true" if enable_dapr else "false"
        }
        annotations = {
            "description" = "CNCF Platform Stack: Prometheus, Grafana, Jaeger, OpenTelemetry, Loki, KEDA, FluxCD, Dapr"
        }
    }
}

# Combine all observability resources
_observability_items = [
    _namespace
] + (prom.prometheus_items if enable_prometheus else []) \
  + (graf.grafana_items if enable_grafana else []) \
  + (jaeg.jaeger_items if enable_jaeger else []) \
  + (otel.otel_collector_items if enable_otel else []) \
  + (lok.loki_items if enable_loki else [])

# Combine platform resources (each has its own namespace)
_platform_items = (keda.keda_items if enable_keda else []) \
  + (flux.fluxcd_items if enable_fluxcd else []) \
  + (dapr.dapr_items if enable_dapr else [])

# All items combined
items = _observability_items + _platform_items
