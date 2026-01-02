# KEDA - CNCF Graduated Project
# Kubernetes Event-driven Autoscaling
# https://keda.sh/

import k8s.api.core.v1 as corev1
import k8s.api.apps.v1 as appsv1
import k8s.api.rbac.v1 as rbacv1

KEDA_VERSION = "2.14.0"
KEDA_IMAGE = "ghcr.io/kedacore/keda:${KEDA_VERSION}"
KEDA_METRICS_IMAGE = "ghcr.io/kedacore/keda-metrics-apiserver:${KEDA_VERSION}"
NAMESPACE = "keda"

# KEDA Namespace
_keda_namespace = corev1.Namespace {
    metadata = {
        name = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "keda"
            "app.kubernetes.io/managed-by" = "kcl"
            "cncf.io/project" = "graduated"
        }
    }
}

# KEDA ServiceAccount
_keda_sa = corev1.ServiceAccount {
    metadata = {
        name = "keda-operator"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "keda-operator"
            "app.kubernetes.io/component" = "operator"
        }
    }
}

# KEDA Metrics Server ServiceAccount
_keda_metrics_sa = corev1.ServiceAccount {
    metadata = {
        name = "keda-metrics-server"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "keda-metrics-server"
        }
    }
}

# KEDA ClusterRole
_keda_cluster_role = rbacv1.ClusterRole {
    metadata = {
        name = "keda-operator"
        labels = {
            "app.kubernetes.io/name" = "keda-operator"
        }
    }
    rules = [
        {
            apiGroups = [""]
            resources = ["configmaps", "configmaps/status", "events", "external", "pods", "secrets", "services", "serviceaccounts"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        {
            apiGroups = ["apps"]
            resources = ["deployments", "statefulsets", "replicasets"]
            verbs = ["get", "list", "watch", "update", "patch"]
        }
        {
            apiGroups = ["apps"]
            resources = ["deployments/scale", "statefulsets/scale"]
            verbs = ["get", "list", "watch", "update", "patch"]
        }
        {
            apiGroups = ["batch"]
            resources = ["jobs"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        {
            apiGroups = ["keda.sh"]
            resources = ["*"]
            verbs = ["*"]
        }
        {
            apiGroups = ["autoscaling"]
            resources = ["horizontalpodautoscalers"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        {
            apiGroups = ["coordination.k8s.io"]
            resources = ["leases"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        # For Dapr integration
        {
            apiGroups = ["dapr.io"]
            resources = ["components", "subscriptions"]
            verbs = ["get", "list", "watch"]
        }
    ]
}

# KEDA ClusterRoleBinding
_keda_cluster_role_binding = rbacv1.ClusterRoleBinding {
    metadata = {
        name = "keda-operator"
        labels = {
            "app.kubernetes.io/name" = "keda-operator"
        }
    }
    roleRef = {
        apiGroup = "rbac.authorization.k8s.io"
        kind = "ClusterRole"
        name = "keda-operator"
    }
    subjects = [
        {
            kind = "ServiceAccount"
            name = "keda-operator"
            namespace = NAMESPACE
        }
    ]
}

# KEDA Metrics Server ClusterRole
_keda_metrics_cluster_role = rbacv1.ClusterRole {
    metadata = {
        name = "keda-metrics-server"
        labels = {
            "app.kubernetes.io/name" = "keda-metrics-server"
        }
    }
    rules = [
        {
            apiGroups = ["external.metrics.k8s.io"]
            resources = ["*"]
            verbs = ["*"]
        }
        {
            apiGroups = ["keda.sh"]
            resources = ["scaledobjects", "scaledobjects/status", "scaledobjects/finalizers", "scaledjobs", "scaledjobs/status", "scaledjobs/finalizers", "triggerauthentications", "triggerauthentications/status", "clustertriggerauthentications", "clustertriggerauthentications/status"]
            verbs = ["get", "list", "watch", "update", "patch"]
        }
        {
            apiGroups = [""]
            resources = ["pods", "services", "secrets", "configmaps"]
            verbs = ["get", "list", "watch"]
        }
    ]
}

# KEDA Metrics Server ClusterRoleBinding
_keda_metrics_cluster_role_binding = rbacv1.ClusterRoleBinding {
    metadata = {
        name = "keda-metrics-server"
        labels = {
            "app.kubernetes.io/name" = "keda-metrics-server"
        }
    }
    roleRef = {
        apiGroup = "rbac.authorization.k8s.io"
        kind = "ClusterRole"
        name = "keda-metrics-server"
    }
    subjects = [
        {
            kind = "ServiceAccount"
            name = "keda-metrics-server"
            namespace = NAMESPACE
        }
    ]
}

# KEDA Operator Deployment
_keda_operator_deployment = appsv1.Deployment {
    metadata = {
        name = "keda-operator"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "keda-operator"
            "app.kubernetes.io/component" = "operator"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "keda-operator"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "keda-operator"
            }
            spec = {
                serviceAccountName = "keda-operator"
                securityContext = {
                    runAsNonRoot = True
                }
                containers = [
                    {
                        name = "keda-operator"
                        image = KEDA_IMAGE
                        command = ["/keda"]
                        args = [
                            "--leader-elect"
                            "--zap-log-level=info"
                            "--zap-encoder=console"
                        ]
                        ports = [
                            {
                                name = "metrics"
                                containerPort = 8080
                                protocol = "TCP"
                            }
                            {
                                name = "health"
                                containerPort = 8081
                                protocol = "TCP"
                            }
                        ]
                        env = [
                            {
                                name = "WATCH_NAMESPACE"
                                value = ""
                            }
                            {
                                name = "POD_NAME"
                                valueFrom.fieldRef.fieldPath = "metadata.name"
                            }
                            {
                                name = "POD_NAMESPACE"
                                valueFrom.fieldRef.fieldPath = "metadata.namespace"
                            }
                            {
                                name = "OPERATOR_NAME"
                                value = "keda-operator"
                            }
                            {
                                name = "KEDA_HTTP_DEFAULT_TIMEOUT"
                                value = "3000"
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "100m"
                                memory = "100Mi"
                            }
                            limits = {
                                cpu = "1000m"
                                memory = "1000Mi"
                            }
                        }
                        livenessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = 8081
                            }
                            initialDelaySeconds = 25
                            periodSeconds = 10
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/readyz"
                                port = 8081
                            }
                            initialDelaySeconds = 20
                            periodSeconds = 5
                        }
                        securityContext = {
                            allowPrivilegeEscalation = False
                            readOnlyRootFilesystem = True
                            capabilities.drop = ["ALL"]
                        }
                    }
                ]
            }
        }
    }
}

# KEDA Metrics Server Deployment
_keda_metrics_deployment = appsv1.Deployment {
    metadata = {
        name = "keda-metrics-apiserver"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "keda-metrics-apiserver"
            "app.kubernetes.io/component" = "metrics"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "keda-metrics-apiserver"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "keda-metrics-apiserver"
            }
            spec = {
                serviceAccountName = "keda-metrics-server"
                securityContext = {
                    runAsNonRoot = True
                }
                containers = [
                    {
                        name = "keda-metrics-apiserver"
                        image = KEDA_METRICS_IMAGE
                        args = [
                            "--secure-port=6443"
                            "--logtostderr=true"
                            "--v=0"
                        ]
                        ports = [
                            {
                                name = "https"
                                containerPort = 6443
                                protocol = "TCP"
                            }
                            {
                                name = "metrics"
                                containerPort = 8080
                                protocol = "TCP"
                            }
                        ]
                        env = [
                            {
                                name = "WATCH_NAMESPACE"
                                value = ""
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "100m"
                                memory = "100Mi"
                            }
                            limits = {
                                cpu = "1000m"
                                memory = "1000Mi"
                            }
                        }
                        livenessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = 6443
                                scheme = "HTTPS"
                            }
                            initialDelaySeconds = 10
                            periodSeconds = 10
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/readyz"
                                port = 6443
                                scheme = "HTTPS"
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 5
                        }
                        securityContext = {
                            allowPrivilegeEscalation = False
                            readOnlyRootFilesystem = True
                            capabilities.drop = ["ALL"]
                        }
                    }
                ]
            }
        }
    }
}

# KEDA Operator Service
_keda_operator_service = corev1.Service {
    metadata = {
        name = "keda-operator"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "keda-operator"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "keda-operator"
        }
        ports = [
            {
                name = "metrics"
                port = 8080
                targetPort = 8080
                protocol = "TCP"
            }
        ]
    }
}

# KEDA Metrics API Service
_keda_metrics_service = corev1.Service {
    metadata = {
        name = "keda-metrics-apiserver"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "keda-metrics-apiserver"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "keda-metrics-apiserver"
        }
        ports = [
            {
                name = "https"
                port = 443
                targetPort = 6443
                protocol = "TCP"
            }
            {
                name = "metrics"
                port = 8080
                targetPort = 8080
                protocol = "TCP"
            }
        ]
    }
}

# Example ScaledObject CRD schema (for reference)
schema ScaledObject:
    """KEDA ScaledObject for event-driven autoscaling"""
    apiVersion: str = "keda.sh/v1alpha1"
    kind: str = "ScaledObject"
    metadata: {str:any}
    spec: {str:any}

# Example: Prometheus-based ScaledObject
_example_prometheus_scaler = ScaledObject {
    metadata = {
        name = "prometheus-scaledobject"
        namespace = "default"
        labels = {
            "app.kubernetes.io/name" = "example-app"
        }
    }
    spec = {
        scaleTargetRef = {
            name = "example-deployment"
        }
        pollingInterval = 30
        cooldownPeriod = 300
        minReplicaCount = 1
        maxReplicaCount = 10
        triggers = [
            {
                $type = "prometheus"
                metadata = {
                    serverAddress = "http://prometheus.monitoring:9090"
                    metricName = "http_requests_total"
                    query = "sum(rate(http_requests_total{deployment=\"example-deployment\"}[2m]))"
                    threshold = "100"
                }
            }
        ]
    }
}

# Example: Dapr pubsub-based ScaledObject
_example_dapr_scaler = ScaledObject {
    metadata = {
        name = "dapr-pubsub-scaledobject"
        namespace = "default"
        labels = {
            "app.kubernetes.io/name" = "dapr-subscriber"
        }
    }
    spec = {
        scaleTargetRef = {
            name = "dapr-subscriber"
        }
        pollingInterval = 15
        cooldownPeriod = 300
        minReplicaCount = 0
        maxReplicaCount = 10
        triggers = [
            {
                $type = "redis"
                metadata = {
                    address = "redis.data:6379"
                    listName = "dapr-messages"
                    listLength = "5"
                }
            }
        ]
    }
}

keda_items = [
    _keda_namespace
    _keda_sa
    _keda_metrics_sa
    _keda_cluster_role
    _keda_cluster_role_binding
    _keda_metrics_cluster_role
    _keda_metrics_cluster_role_binding
    _keda_operator_deployment
    _keda_metrics_deployment
    _keda_operator_service
    _keda_metrics_service
]

# Example scalers (optional - not included by default)
keda_example_scalers = [
    _example_prometheus_scaler
    _example_dapr_scaler
]
