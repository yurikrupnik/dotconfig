# Prometheus - CNCF Graduated Project
# Metrics collection and alerting system
# https://prometheus.io/

import k8s.api.core.v1 as corev1
import k8s.api.apps.v1 as appsv1
import k8s.api.rbac.v1 as rbacv1

PROMETHEUS_VERSION = "v2.51.0"
PROMETHEUS_IMAGE = "prom/prometheus:${PROMETHEUS_VERSION}"
NAMESPACE = "monitoring"

# Prometheus ServiceAccount
_prometheus_sa = corev1.ServiceAccount {
    metadata = {
        name = "prometheus"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "prometheus"
            "app.kubernetes.io/component" = "metrics"
            "cncf.io/project" = "graduated"
        }
    }
}

# Prometheus ClusterRole
_prometheus_cluster_role = rbacv1.ClusterRole {
    metadata = {
        name = "prometheus"
        labels = {
            "app.kubernetes.io/name" = "prometheus"
        }
    }
    rules = [
        {
            apiGroups = [""]
            resources = ["nodes", "nodes/proxy", "nodes/metrics", "services", "endpoints", "pods"]
            verbs = ["get", "list", "watch"]
        }
        {
            apiGroups = ["extensions", "networking.k8s.io"]
            resources = ["ingresses"]
            verbs = ["get", "list", "watch"]
        }
        {
            nonResourceURLs = ["/metrics", "/metrics/cadvisor"]
            verbs = ["get"]
        }
    ]
}

# Prometheus ClusterRoleBinding
_prometheus_cluster_role_binding = rbacv1.ClusterRoleBinding {
    metadata = {
        name = "prometheus"
        labels = {
            "app.kubernetes.io/name" = "prometheus"
        }
    }
    roleRef = {
        apiGroup = "rbac.authorization.k8s.io"
        kind = "ClusterRole"
        name = "prometheus"
    }
    subjects = [
        {
            kind = "ServiceAccount"
            name = "prometheus"
            namespace = NAMESPACE
        }
    ]
}

# Prometheus ConfigMap
_prometheus_config = corev1.ConfigMap {
    metadata = {
        name = "prometheus-config"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "prometheus"
        }
    }
    data = {
        "prometheus.yml" = """\
global:
  scrape_interval: 15s
  evaluation_interval: 15s

alerting:
  alertmanagers:
    - static_configs:
        - targets: []

rule_files: []

scrape_configs:
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']

  - job_name: 'kubernetes-apiservers'
    kubernetes_sd_configs:
      - role: endpoints
    scheme: https
    tls_config:
      ca_file: /var/run/secrets/kubernetes.io/serviceaccount/ca.crt
    bearer_token_file: /var/run/secrets/kubernetes.io/serviceaccount/token
    relabel_configs:
      - source_labels: [__meta_kubernetes_namespace, __meta_kubernetes_service_name, __meta_kubernetes_endpoint_port_name]
        action: keep
        regex: default;kubernetes;https

  - job_name: 'kubernetes-nodes'
    scheme: https
    tls_config:
      ca_file: /var/run/secrets/kubernetes.io/serviceaccount/ca.crt
    bearer_token_file: /var/run/secrets/kubernetes.io/serviceaccount/token
    kubernetes_sd_configs:
      - role: node
    relabel_configs:
      - action: labelmap
        regex: __meta_kubernetes_node_label_(.+)

  - job_name: 'kubernetes-pods'
    kubernetes_sd_configs:
      - role: pod
    relabel_configs:
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_scrape]
        action: keep
        regex: true
      - source_labels: [__meta_kubernetes_pod_annotation_prometheus_io_path]
        action: replace
        target_label: __metrics_path__
        regex: (.+)
      - source_labels: [__address__, __meta_kubernetes_pod_annotation_prometheus_io_port]
        action: replace
        regex: ([^:]+)(?::\d+)?;(\d+)
        replacement: $1:$2
        target_label: __address__
      - action: labelmap
        regex: __meta_kubernetes_pod_label_(.+)
      - source_labels: [__meta_kubernetes_namespace]
        action: replace
        target_label: kubernetes_namespace
      - source_labels: [__meta_kubernetes_pod_name]
        action: replace
        target_label: kubernetes_pod_name

  - job_name: 'kubernetes-services'
    metrics_path: /probe
    params:
      module: [http_2xx]
    kubernetes_sd_configs:
      - role: service
    relabel_configs:
      - source_labels: [__meta_kubernetes_service_annotation_prometheus_io_probe]
        action: keep
        regex: true
      - source_labels: [__address__]
        target_label: __param_target
      - target_label: __address__
        replacement: blackbox-exporter:9115
      - source_labels: [__param_target]
        target_label: instance
      - action: labelmap
        regex: __meta_kubernetes_service_label_(.+)
      - source_labels: [__meta_kubernetes_namespace]
        target_label: kubernetes_namespace
      - source_labels: [__meta_kubernetes_service_name]
        target_label: kubernetes_name

  # OpenTelemetry Collector metrics
  - job_name: 'otel-collector'
    static_configs:
      - targets: ['otel-collector.monitoring:8888']

  # Jaeger metrics
  - job_name: 'jaeger'
    static_configs:
      - targets: ['jaeger.monitoring:14269']

  # Loki metrics
  - job_name: 'loki'
    static_configs:
      - targets: ['loki.monitoring:3100']
"""
    }
}

# Prometheus Deployment
_prometheus_deployment = appsv1.Deployment {
    metadata = {
        name = "prometheus"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "prometheus"
            "app.kubernetes.io/component" = "metrics"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "prometheus"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "prometheus"
            }
            spec = {
                serviceAccountName = "prometheus"
                securityContext = {
                    fsGroup = 65534
                    runAsNonRoot = True
                    runAsUser = 65534
                }
                containers = [
                    {
                        name = "prometheus"
                        image = PROMETHEUS_IMAGE
                        args = [
                            "--config.file=/etc/prometheus/prometheus.yml"
                            "--storage.tsdb.path=/prometheus"
                            "--storage.tsdb.retention.time=15d"
                            "--web.enable-lifecycle"
                            "--web.enable-remote-write-receiver"
                        ]
                        ports = [
                            {
                                name = "http"
                                containerPort = 9090
                                protocol = "TCP"
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "250m"
                                memory = "512Mi"
                            }
                            limits = {
                                cpu = "1000m"
                                memory = "2Gi"
                            }
                        }
                        volumeMounts = [
                            {
                                name = "config"
                                mountPath = "/etc/prometheus"
                            }
                            {
                                name = "data"
                                mountPath = "/prometheus"
                            }
                        ]
                        livenessProbe = {
                            httpGet = {
                                path = "/-/healthy"
                                port = 9090
                            }
                            initialDelaySeconds = 30
                            periodSeconds = 10
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/-/ready"
                                port = 9090
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 5
                        }
                    }
                ]
                volumes = [
                    {
                        name = "config"
                        configMap.name = "prometheus-config"
                    }
                    {
                        name = "data"
                        emptyDir = {}
                    }
                ]
            }
        }
    }
}

# Prometheus Service
_prometheus_service = corev1.Service {
    metadata = {
        name = "prometheus"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "prometheus"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "prometheus"
        }
        ports = [
            {
                name = "http"
                port = 9090
                targetPort = 9090
                protocol = "TCP"
            }
        ]
    }
}

prometheus_items = [
    _prometheus_sa
    _prometheus_cluster_role
    _prometheus_cluster_role_binding
    _prometheus_config
    _prometheus_deployment
    _prometheus_service
]
