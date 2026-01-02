# Loki - Grafana Labs / CNCF Ecosystem
# Log Aggregation System (Like Prometheus but for Logs)
# https://grafana.com/oss/loki/

import k8s.api.core.v1 as corev1
import k8s.api.apps.v1 as appsv1

LOKI_VERSION = "3.0.0"
LOKI_IMAGE = "grafana/loki:${LOKI_VERSION}"
NAMESPACE = "monitoring"

# Loki ConfigMap
_loki_config = corev1.ConfigMap {
    metadata = {
        name = "loki-config"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "loki"
            "app.kubernetes.io/component" = "logging"
        }
    }
    data = {
        "loki.yaml" = """\
auth_enabled: false

server:
  http_listen_port: 3100
  grpc_listen_port: 9096
  log_level: info

common:
  instance_addr: 127.0.0.1
  path_prefix: /loki
  storage:
    filesystem:
      chunks_directory: /loki/chunks
      rules_directory: /loki/rules
  replication_factor: 1
  ring:
    kvstore:
      store: inmemory

query_range:
  results_cache:
    cache:
      embedded_cache:
        enabled: true
        max_size_mb: 100

schema_config:
  configs:
    - from: 2020-10-24
      store: tsdb
      object_store: filesystem
      schema: v13
      index:
        prefix: index_
        period: 24h

ruler:
  alertmanager_url: http://localhost:9093
  storage:
    type: local
    local:
      directory: /loki/rules
  rule_path: /loki/rules-temp
  enable_api: true

ingester:
  lifecycler:
    ring:
      replication_factor: 1
  chunk_idle_period: 5m
  chunk_retain_period: 30s
  max_transfer_retries: 0

limits_config:
  retention_period: 744h  # 31 days
  enforce_metric_name: false
  reject_old_samples: true
  reject_old_samples_max_age: 168h
  max_entries_limit_per_query: 5000
  ingestion_rate_mb: 10
  ingestion_burst_size_mb: 20
  per_stream_rate_limit: 5MB
  per_stream_rate_limit_burst: 15MB

compactor:
  working_directory: /loki/compactor
  compaction_interval: 10m
  retention_enabled: true
  retention_delete_delay: 2h
  retention_delete_worker_count: 150
  delete_request_store: filesystem

table_manager:
  retention_deletes_enabled: true
  retention_period: 744h

analytics:
  reporting_enabled: false
"""
    }
}

# Loki StatefulSet (for data persistence)
_loki_statefulset = appsv1.StatefulSet {
    metadata = {
        name = "loki"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "loki"
            "app.kubernetes.io/component" = "logging"
        }
    }
    spec = {
        serviceName = "loki"
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "loki"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "loki"
            }
            spec = {
                securityContext = {
                    fsGroup = 10001
                    runAsUser = 10001
                    runAsGroup = 10001
                    runAsNonRoot = True
                }
                containers = [
                    {
                        name = "loki"
                        image = LOKI_IMAGE
                        args = ["-config.file=/etc/loki/loki.yaml"]
                        ports = [
                            {
                                name = "http"
                                containerPort = 3100
                                protocol = "TCP"
                            }
                            {
                                name = "grpc"
                                containerPort = 9096
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
                                memory = "1Gi"
                            }
                        }
                        volumeMounts = [
                            {
                                name = "config"
                                mountPath = "/etc/loki"
                            }
                            {
                                name = "data"
                                mountPath = "/loki"
                            }
                        ]
                        livenessProbe = {
                            httpGet = {
                                path = "/ready"
                                port = 3100
                            }
                            initialDelaySeconds = 45
                            periodSeconds = 10
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/ready"
                                port = 3100
                            }
                            initialDelaySeconds = 15
                            periodSeconds = 5
                        }
                        securityContext = {
                            allowPrivilegeEscalation = False
                            readOnlyRootFilesystem = True
                            capabilities.drop = ["ALL"]
                        }
                    }
                ]
                volumes = [
                    {
                        name = "config"
                        configMap.name = "loki-config"
                    }
                ]
            }
        }
        volumeClaimTemplates = [
            {
                metadata.name = "data"
                spec = {
                    accessModes = ["ReadWriteOnce"]
                    resources.requests.storage = "10Gi"
                }
            }
        ]
    }
}

# Loki Service
_loki_service = corev1.Service {
    metadata = {
        name = "loki"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "loki"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "loki"
        }
        ports = [
            {
                name = "http"
                port = 3100
                targetPort = 3100
                protocol = "TCP"
            }
            {
                name = "grpc"
                port = 9096
                targetPort = 9096
                protocol = "TCP"
            }
        ]
    }
}

# Promtail DaemonSet for log collection
_promtail_config = corev1.ConfigMap {
    metadata = {
        name = "promtail-config"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "promtail"
        }
    }
    data = {
        "promtail.yaml" = """\
server:
  http_listen_port: 3101
  grpc_listen_port: 0

positions:
  filename: /run/promtail/positions.yaml

clients:
  - url: http://loki.monitoring:3100/loki/api/v1/push
    batchwait: 1s
    batchsize: 1048576
    timeout: 10s

scrape_configs:
  - job_name: kubernetes-pods
    kubernetes_sd_configs:
      - role: pod
    pipeline_stages:
      - cri: {}
      - multiline:
          firstline: '^\\d{4}-\\d{2}-\\d{2}|^\\[\\d{4}'
          max_wait_time: 3s
      - json:
          expressions:
            level: level
            msg: msg
            trace_id: trace_id
          source: ""
      - labels:
          level:
          trace_id:
    relabel_configs:
      - source_labels: [__meta_kubernetes_pod_controller_name]
        regex: ([0-9a-z-.]+?)(-[0-9a-f]{8,10})?
        action: replace
        target_label: __tmp_controller_name
      - source_labels: [__meta_kubernetes_pod_label_app_kubernetes_io_name, __meta_kubernetes_pod_label_app, __tmp_controller_name, __meta_kubernetes_pod_name]
        regex: ^;*([^;]+)(;.*)?$
        action: replace
        target_label: app
      - source_labels: [__meta_kubernetes_pod_label_app_kubernetes_io_instance, __meta_kubernetes_pod_label_instance]
        regex: ^;*([^;]+)(;.*)?$
        action: replace
        target_label: instance
      - source_labels: [__meta_kubernetes_pod_label_app_kubernetes_io_component, __meta_kubernetes_pod_label_component]
        regex: ^;*([^;]+)(;.*)?$
        action: replace
        target_label: component
      - action: replace
        source_labels: [__meta_kubernetes_pod_node_name]
        target_label: node_name
      - action: replace
        source_labels: [__meta_kubernetes_namespace]
        target_label: namespace
      - action: replace
        replacement: $1
        separator: /
        source_labels: [namespace, app]
        target_label: job
      - action: replace
        source_labels: [__meta_kubernetes_pod_name]
        target_label: pod
      - action: replace
        source_labels: [__meta_kubernetes_pod_container_name]
        target_label: container
      - action: replace
        replacement: /var/log/pods/*$1/*.log
        separator: /
        source_labels: [__meta_kubernetes_pod_uid, __meta_kubernetes_pod_container_name]
        target_label: __path__
      - action: replace
        regex: true/(.*)
        replacement: /var/log/pods/*$1/*.log
        separator: /
        source_labels: [__meta_kubernetes_pod_annotationpresent_kubernetes_io_config_hash, __meta_kubernetes_pod_annotation_kubernetes_io_config_hash, __meta_kubernetes_pod_container_name]
        target_label: __path__
"""
    }
}

_promtail_daemonset = appsv1.DaemonSet {
    metadata = {
        name = "promtail"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "promtail"
            "app.kubernetes.io/component" = "log-collector"
        }
    }
    spec = {
        selector.matchLabels = {
            "app.kubernetes.io/name" = "promtail"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "promtail"
            }
            spec = {
                serviceAccountName = "promtail"
                containers = [
                    {
                        name = "promtail"
                        image = "grafana/promtail:${LOKI_VERSION}"
                        args = ["-config.file=/etc/promtail/promtail.yaml"]
                        ports = [
                            {
                                name = "http"
                                containerPort = 3101
                                protocol = "TCP"
                            }
                        ]
                        env = [
                            {
                                name = "HOSTNAME"
                                valueFrom.fieldRef.fieldPath = "spec.nodeName"
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "50m"
                                memory = "64Mi"
                            }
                            limits = {
                                cpu = "200m"
                                memory = "128Mi"
                            }
                        }
                        volumeMounts = [
                            {
                                name = "config"
                                mountPath = "/etc/promtail"
                            }
                            {
                                name = "run"
                                mountPath = "/run/promtail"
                            }
                            {
                                name = "pods"
                                mountPath = "/var/log/pods"
                                readOnly = True
                            }
                            {
                                name = "containers"
                                mountPath = "/var/lib/docker/containers"
                                readOnly = True
                            }
                        ]
                        securityContext = {
                            allowPrivilegeEscalation = False
                            capabilities.drop = ["ALL"]
                            readOnlyRootFilesystem = True
                        }
                    }
                ]
                tolerations = [
                    {
                        effect = "NoSchedule"
                        operator = "Exists"
                    }
                ]
                volumes = [
                    {
                        name = "config"
                        configMap.name = "promtail-config"
                    }
                    {
                        name = "run"
                        hostPath = {
                            path = "/run/promtail"
                        }
                    }
                    {
                        name = "pods"
                        hostPath = {
                            path = "/var/log/pods"
                        }
                    }
                    {
                        name = "containers"
                        hostPath = {
                            path = "/var/lib/docker/containers"
                        }
                    }
                ]
            }
        }
    }
}

_promtail_sa = corev1.ServiceAccount {
    metadata = {
        name = "promtail"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "promtail"
        }
    }
}

import k8s.api.rbac.v1 as rbacv1

_promtail_cluster_role = rbacv1.ClusterRole {
    metadata = {
        name = "promtail"
        labels = {
            "app.kubernetes.io/name" = "promtail"
        }
    }
    rules = [
        {
            apiGroups = [""]
            resources = ["nodes", "nodes/proxy", "services", "endpoints", "pods"]
            verbs = ["get", "list", "watch"]
        }
    ]
}

_promtail_cluster_role_binding = rbacv1.ClusterRoleBinding {
    metadata = {
        name = "promtail"
        labels = {
            "app.kubernetes.io/name" = "promtail"
        }
    }
    roleRef = {
        apiGroup = "rbac.authorization.k8s.io"
        kind = "ClusterRole"
        name = "promtail"
    }
    subjects = [
        {
            kind = "ServiceAccount"
            name = "promtail"
            namespace = NAMESPACE
        }
    ]
}

loki_items = [
    _loki_config
    _loki_statefulset
    _loki_service
    _promtail_sa
    _promtail_cluster_role
    _promtail_cluster_role_binding
    _promtail_config
    _promtail_daemonset
]
