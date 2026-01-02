# Grafana - Observability UI Management Layer
# Unified dashboard for Prometheus, Loki, Jaeger, and OpenTelemetry
# https://grafana.com/

import k8s.api.core.v1 as corev1
import k8s.api.apps.v1 as appsv1

GRAFANA_VERSION = "11.0.0"
GRAFANA_IMAGE = "grafana/grafana:${GRAFANA_VERSION}"
NAMESPACE = "monitoring"

# Grafana ConfigMap for datasources
_grafana_datasources = corev1.ConfigMap {
    metadata = {
        name = "grafana-datasources"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "grafana"
            "app.kubernetes.io/component" = "ui"
        }
    }
    data = {
        "datasources.yaml" = """\
apiVersion: 1
datasources:
  # Prometheus - Metrics
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://prometheus.monitoring:9090
    isDefault: true
    editable: false
    jsonData:
      httpMethod: POST
      manageAlerts: true
      prometheusType: Prometheus
      prometheusVersion: "2.51.0"

  # Loki - Logs
  - name: Loki
    type: loki
    access: proxy
    url: http://loki.monitoring:3100
    editable: false
    jsonData:
      maxLines: 1000
      derivedFields:
        - datasourceUid: jaeger
          matcherRegex: "trace_id=(\\\\w+)"
          name: TraceID
          url: '$__value.raw'

  # Jaeger - Traces
  - name: Jaeger
    type: jaeger
    uid: jaeger
    access: proxy
    url: http://jaeger.monitoring:16686
    editable: false
    jsonData:
      tracesToLogsV2:
        datasourceUid: 'loki'
        spanStartTimeShift: '-1h'
        spanEndTimeShift: '1h'
        filterByTraceID: true
        filterBySpanID: true

  # OpenTelemetry - via Prometheus remote write
  - name: OpenTelemetry Metrics
    type: prometheus
    access: proxy
    url: http://otel-collector.monitoring:8889
    editable: false
    jsonData:
      httpMethod: POST
"""
    }
}

# Grafana ConfigMap for dashboards provisioning
_grafana_dashboard_provider = corev1.ConfigMap {
    metadata = {
        name = "grafana-dashboard-provider"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "grafana"
        }
    }
    data = {
        "dashboards.yaml" = """\
apiVersion: 1
providers:
  - name: 'default'
    orgId: 1
    folder: ''
    type: file
    disableDeletion: false
    editable: true
    options:
      path: /var/lib/grafana/dashboards
"""
    }
}

# Grafana ConfigMap for pre-built dashboards
_grafana_dashboards = corev1.ConfigMap {
    metadata = {
        name = "grafana-dashboards"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "grafana"
        }
    }
    data = {
        "kubernetes-overview.json" = """\
{
  "annotations": {
    "list": []
  },
  "editable": true,
  "fiscalYearStartMonth": 0,
  "graphTooltip": 0,
  "id": null,
  "links": [],
  "liveNow": false,
  "panels": [
    {
      "datasource": { "type": "prometheus", "uid": "prometheus" },
      "fieldConfig": {
        "defaults": { "color": { "mode": "palette-classic" }, "unit": "short" }
      },
      "gridPos": { "h": 8, "w": 12, "x": 0, "y": 0 },
      "title": "Pod Count by Namespace",
      "type": "timeseries",
      "targets": [
        {
          "expr": "count(kube_pod_info) by (namespace)",
          "refId": "A"
        }
      ]
    },
    {
      "datasource": { "type": "prometheus", "uid": "prometheus" },
      "fieldConfig": {
        "defaults": { "color": { "mode": "palette-classic" }, "unit": "percent" }
      },
      "gridPos": { "h": 8, "w": 12, "x": 12, "y": 0 },
      "title": "CPU Usage by Pod",
      "type": "timeseries",
      "targets": [
        {
          "expr": "sum(rate(container_cpu_usage_seconds_total[5m])) by (pod) * 100",
          "refId": "A"
        }
      ]
    },
    {
      "datasource": { "type": "loki", "uid": "loki" },
      "gridPos": { "h": 8, "w": 24, "x": 0, "y": 8 },
      "title": "Application Logs",
      "type": "logs",
      "targets": [
        {
          "expr": "{namespace=~\".+\"}",
          "refId": "A"
        }
      ]
    }
  ],
  "schemaVersion": 38,
  "tags": ["kubernetes", "overview"],
  "templating": { "list": [] },
  "time": { "from": "now-1h", "to": "now" },
  "title": "Kubernetes Overview",
  "uid": "k8s-overview"
}
"""
        "observability-stack.json" = """\
{
  "annotations": { "list": [] },
  "editable": true,
  "panels": [
    {
      "datasource": { "type": "prometheus", "uid": "prometheus" },
      "gridPos": { "h": 4, "w": 6, "x": 0, "y": 0 },
      "title": "Prometheus Status",
      "type": "stat",
      "targets": [{ "expr": "up{job=\\"prometheus\\"}", "refId": "A" }]
    },
    {
      "datasource": { "type": "prometheus", "uid": "prometheus" },
      "gridPos": { "h": 4, "w": 6, "x": 6, "y": 0 },
      "title": "Loki Status",
      "type": "stat",
      "targets": [{ "expr": "up{job=\\"loki\\"}", "refId": "A" }]
    },
    {
      "datasource": { "type": "prometheus", "uid": "prometheus" },
      "gridPos": { "h": 4, "w": 6, "x": 12, "y": 0 },
      "title": "Jaeger Status",
      "type": "stat",
      "targets": [{ "expr": "up{job=\\"jaeger\\"}", "refId": "A" }]
    },
    {
      "datasource": { "type": "prometheus", "uid": "prometheus" },
      "gridPos": { "h": 4, "w": 6, "x": 18, "y": 0 },
      "title": "OTel Collector Status",
      "type": "stat",
      "targets": [{ "expr": "up{job=\\"otel-collector\\"}", "refId": "A" }]
    },
    {
      "datasource": { "type": "prometheus", "uid": "prometheus" },
      "gridPos": { "h": 8, "w": 12, "x": 0, "y": 4 },
      "title": "Prometheus Scrape Duration",
      "type": "timeseries",
      "targets": [{ "expr": "scrape_duration_seconds", "refId": "A" }]
    },
    {
      "datasource": { "type": "prometheus", "uid": "prometheus" },
      "gridPos": { "h": 8, "w": 12, "x": 12, "y": 4 },
      "title": "OTel Collector Received Spans",
      "type": "timeseries",
      "targets": [{ "expr": "otelcol_receiver_accepted_spans", "refId": "A" }]
    }
  ],
  "schemaVersion": 38,
  "tags": ["observability", "cncf"],
  "time": { "from": "now-1h", "to": "now" },
  "title": "CNCF Observability Stack",
  "uid": "cncf-observability"
}
"""
    }
}

# Grafana Secret for admin credentials
_grafana_secret = corev1.Secret {
    metadata = {
        name = "grafana-credentials"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "grafana"
        }
    }
    $type = "Opaque"
    stringData = {
        "admin-user" = "admin"
        "admin-password" = "admin"  # Change in production!
    }
}

# Grafana Deployment
_grafana_deployment = appsv1.Deployment {
    metadata = {
        name = "grafana"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "grafana"
            "app.kubernetes.io/component" = "ui"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "grafana"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "grafana"
            }
            spec = {
                securityContext = {
                    fsGroup = 472
                    runAsUser = 472
                    runAsNonRoot = True
                }
                containers = [
                    {
                        name = "grafana"
                        image = GRAFANA_IMAGE
                        ports = [
                            {
                                name = "http"
                                containerPort = 3000
                                protocol = "TCP"
                            }
                        ]
                        env = [
                            {
                                name = "GF_SECURITY_ADMIN_USER"
                                valueFrom.secretKeyRef = {
                                    name = "grafana-credentials"
                                    key = "admin-user"
                                }
                            }
                            {
                                name = "GF_SECURITY_ADMIN_PASSWORD"
                                valueFrom.secretKeyRef = {
                                    name = "grafana-credentials"
                                    key = "admin-password"
                                }
                            }
                            {
                                name = "GF_INSTALL_PLUGINS"
                                value = "grafana-clock-panel,grafana-simple-json-datasource,grafana-piechart-panel"
                            }
                            {
                                name = "GF_FEATURE_TOGGLES_ENABLE"
                                value = "traceqlEditor"
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
                                name = "datasources"
                                mountPath = "/etc/grafana/provisioning/datasources"
                            }
                            {
                                name = "dashboard-provider"
                                mountPath = "/etc/grafana/provisioning/dashboards"
                            }
                            {
                                name = "dashboards"
                                mountPath = "/var/lib/grafana/dashboards"
                            }
                            {
                                name = "data"
                                mountPath = "/var/lib/grafana"
                            }
                        ]
                        livenessProbe = {
                            httpGet = {
                                path = "/api/health"
                                port = 3000
                            }
                            initialDelaySeconds = 60
                            periodSeconds = 10
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/api/health"
                                port = 3000
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 5
                        }
                    }
                ]
                volumes = [
                    {
                        name = "datasources"
                        configMap.name = "grafana-datasources"
                    }
                    {
                        name = "dashboard-provider"
                        configMap.name = "grafana-dashboard-provider"
                    }
                    {
                        name = "dashboards"
                        configMap.name = "grafana-dashboards"
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

# Grafana Service
_grafana_service = corev1.Service {
    metadata = {
        name = "grafana"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "grafana"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "grafana"
        }
        ports = [
            {
                name = "http"
                port = 3000
                targetPort = 3000
                protocol = "TCP"
            }
        ]
    }
}

grafana_items = [
    _grafana_datasources
    _grafana_dashboard_provider
    _grafana_dashboards
    _grafana_secret
    _grafana_deployment
    _grafana_service
]
