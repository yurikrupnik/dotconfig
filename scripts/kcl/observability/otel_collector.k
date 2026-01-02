# OpenTelemetry Collector - CNCF Incubating Project
# Unified Telemetry Collection (Metrics, Logs, Traces)
# https://opentelemetry.io/

import k8s.api.core.v1 as corev1
import k8s.api.apps.v1 as appsv1
import k8s.api.rbac.v1 as rbacv1

OTEL_VERSION = "0.102.0"
OTEL_IMAGE = "otel/opentelemetry-collector-contrib:${OTEL_VERSION}"
NAMESPACE = "monitoring"

# OpenTelemetry Collector ServiceAccount
_otel_sa = corev1.ServiceAccount {
    metadata = {
        name = "otel-collector"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "otel-collector"
            "app.kubernetes.io/component" = "telemetry"
            "cncf.io/project" = "incubating"
        }
    }
}

# OpenTelemetry Collector ClusterRole
_otel_cluster_role = rbacv1.ClusterRole {
    metadata = {
        name = "otel-collector"
        labels = {
            "app.kubernetes.io/name" = "otel-collector"
        }
    }
    rules = [
        {
            apiGroups = [""]
            resources = ["nodes", "nodes/proxy", "nodes/metrics", "services", "endpoints", "pods", "events", "namespaces"]
            verbs = ["get", "list", "watch"]
        }
        {
            apiGroups = ["apps"]
            resources = ["replicasets", "deployments", "daemonsets", "statefulsets"]
            verbs = ["get", "list", "watch"]
        }
        {
            apiGroups = ["extensions"]
            resources = ["replicasets", "deployments", "daemonsets"]
            verbs = ["get", "list", "watch"]
        }
        {
            apiGroups = ["batch"]
            resources = ["jobs", "cronjobs"]
            verbs = ["get", "list", "watch"]
        }
        {
            nonResourceURLs = ["/metrics", "/metrics/cadvisor"]
            verbs = ["get"]
        }
    ]
}

# OpenTelemetry Collector ClusterRoleBinding
_otel_cluster_role_binding = rbacv1.ClusterRoleBinding {
    metadata = {
        name = "otel-collector"
        labels = {
            "app.kubernetes.io/name" = "otel-collector"
        }
    }
    roleRef = {
        apiGroup = "rbac.authorization.k8s.io"
        kind = "ClusterRole"
        name = "otel-collector"
    }
    subjects = [
        {
            kind = "ServiceAccount"
            name = "otel-collector"
            namespace = NAMESPACE
        }
    ]
}

# OpenTelemetry Collector ConfigMap
_otel_config = corev1.ConfigMap {
    metadata = {
        name = "otel-collector-config"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "otel-collector"
        }
    }
    data = {
        "config.yaml" = """\
receivers:
  # OTLP receiver for applications sending telemetry
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

  # Prometheus receiver to scrape metrics
  prometheus:
    config:
      scrape_configs:
        - job_name: 'otel-collector'
          scrape_interval: 10s
          static_configs:
            - targets: ['0.0.0.0:8888']

  # Kubernetes cluster metrics
  k8s_cluster:
    collection_interval: 10s
    node_conditions_to_report: [Ready, MemoryPressure]
    allocatable_types_to_report: [cpu, memory]

  # Host metrics
  hostmetrics:
    collection_interval: 30s
    scrapers:
      cpu:
      memory:
      disk:
      network:

processors:
  # Batch processor for performance
  batch:
    timeout: 10s
    send_batch_size: 1024

  # Memory limiter to prevent OOM
  memory_limiter:
    check_interval: 1s
    limit_mib: 400
    spike_limit_mib: 100

  # Resource detection for Kubernetes
  resourcedetection:
    detectors: [env, system, docker]
    timeout: 5s

  # K8s attributes processor
  k8sattributes:
    auth_type: "serviceAccount"
    passthrough: false
    extract:
      metadata:
        - k8s.namespace.name
        - k8s.deployment.name
        - k8s.pod.name
        - k8s.node.name
        - k8s.pod.uid
      labels:
        - tag_name: app.label.component
          key: app.kubernetes.io/component
          from: pod

  # Add resource attributes
  resource:
    attributes:
      - key: service.instance.id
        from_attribute: k8s.pod.uid
        action: insert

exporters:
  # Debug exporter for troubleshooting
  debug:
    verbosity: detailed
    sampling_initial: 5
    sampling_thereafter: 200

  # Prometheus exporter for metrics
  prometheus:
    endpoint: 0.0.0.0:8889
    namespace: otel
    send_timestamps: true
    metric_expiration: 180m
    resource_to_telemetry_conversion:
      enabled: true

  # Prometheus remote write to Prometheus server
  prometheusremotewrite:
    endpoint: http://prometheus.monitoring:9090/api/v1/write
    tls:
      insecure: true

  # OTLP exporter to Jaeger
  otlp/jaeger:
    endpoint: jaeger-collector.monitoring:4317
    tls:
      insecure: true

  # Loki exporter for logs
  loki:
    endpoint: http://loki.monitoring:3100/loki/api/v1/push
    labels:
      resource:
        k8s.namespace.name: "namespace"
        k8s.pod.name: "pod"
        k8s.container.name: "container"
      attributes:
        level: ""
        service.name: "service"

extensions:
  # Health check extension
  health_check:
    endpoint: 0.0.0.0:13133

  # Pprof extension for profiling
  pprof:
    endpoint: 0.0.0.0:1777

  # ZPages for debugging
  zpages:
    endpoint: 0.0.0.0:55679

service:
  extensions: [health_check, pprof, zpages]
  pipelines:
    # Traces pipeline
    traces:
      receivers: [otlp]
      processors: [memory_limiter, batch, k8sattributes, resource]
      exporters: [otlp/jaeger, debug]

    # Metrics pipeline
    metrics:
      receivers: [otlp, prometheus, hostmetrics]
      processors: [memory_limiter, batch, resourcedetection]
      exporters: [prometheus, prometheusremotewrite, debug]

    # Logs pipeline
    logs:
      receivers: [otlp]
      processors: [memory_limiter, batch, k8sattributes, resource]
      exporters: [loki, debug]

  telemetry:
    logs:
      level: info
    metrics:
      address: 0.0.0.0:8888
"""
    }
}

# OpenTelemetry Collector Deployment
_otel_deployment = appsv1.Deployment {
    metadata = {
        name = "otel-collector"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "otel-collector"
            "app.kubernetes.io/component" = "telemetry"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "otel-collector"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "otel-collector"
            }
            spec = {
                serviceAccountName = "otel-collector"
                containers = [
                    {
                        name = "otel-collector"
                        image = OTEL_IMAGE
                        args = ["--config=/etc/otelcol/config.yaml"]
                        ports = [
                            {
                                name = "otlp-grpc"
                                containerPort = 4317
                                protocol = "TCP"
                            }
                            {
                                name = "otlp-http"
                                containerPort = 4318
                                protocol = "TCP"
                            }
                            {
                                name = "prometheus"
                                containerPort = 8889
                                protocol = "TCP"
                            }
                            {
                                name = "metrics"
                                containerPort = 8888
                                protocol = "TCP"
                            }
                            {
                                name = "health"
                                containerPort = 13133
                                protocol = "TCP"
                            }
                            {
                                name = "zpages"
                                containerPort = 55679
                                protocol = "TCP"
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "100m"
                                memory = "256Mi"
                            }
                            limits = {
                                cpu = "500m"
                                memory = "512Mi"
                            }
                        }
                        volumeMounts = [
                            {
                                name = "config"
                                mountPath = "/etc/otelcol"
                            }
                        ]
                        livenessProbe = {
                            httpGet = {
                                path = "/"
                                port = 13133
                            }
                            initialDelaySeconds = 10
                            periodSeconds = 10
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/"
                                port = 13133
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 5
                        }
                    }
                ]
                volumes = [
                    {
                        name = "config"
                        configMap.name = "otel-collector-config"
                    }
                ]
            }
        }
    }
}

# OpenTelemetry Collector Service
_otel_service = corev1.Service {
    metadata = {
        name = "otel-collector"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "otel-collector"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "otel-collector"
        }
        ports = [
            {
                name = "otlp-grpc"
                port = 4317
                targetPort = 4317
                protocol = "TCP"
            }
            {
                name = "otlp-http"
                port = 4318
                targetPort = 4318
                protocol = "TCP"
            }
            {
                name = "prometheus"
                port = 8889
                targetPort = 8889
                protocol = "TCP"
            }
            {
                name = "metrics"
                port = 8888
                targetPort = 8888
                protocol = "TCP"
            }
        ]
    }
}

otel_collector_items = [
    _otel_sa
    _otel_cluster_role
    _otel_cluster_role_binding
    _otel_config
    _otel_deployment
    _otel_service
]
