# Dapr - CNCF Graduated Project
# Distributed Application Runtime
# https://dapr.io/

import k8s.api.core.v1 as corev1
import k8s.api.apps.v1 as appsv1
import k8s.api.rbac.v1 as rbacv1

DAPR_VERSION = "1.13.0"
DAPR_SIDECAR_IMAGE = "daprio/daprd:${DAPR_VERSION}"
DAPR_SENTRY_IMAGE = "daprio/sentry:${DAPR_VERSION}"
DAPR_OPERATOR_IMAGE = "daprio/dapr:${DAPR_VERSION}"
DAPR_PLACEMENT_IMAGE = "daprio/placement:${DAPR_VERSION}"
DAPR_SCHEDULER_IMAGE = "daprio/scheduler:${DAPR_VERSION}"
NAMESPACE = "dapr-system"

# Dapr Namespace
_dapr_namespace = corev1.Namespace {
    metadata = {
        name = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr"
            "app.kubernetes.io/managed-by" = "kcl"
            "cncf.io/project" = "graduated"
        }
    }
}

# Dapr Operator ServiceAccount
_dapr_operator_sa = corev1.ServiceAccount {
    metadata = {
        name = "dapr-operator"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr-operator"
            "app.kubernetes.io/component" = "operator"
        }
    }
}

# Dapr Sentry ServiceAccount
_dapr_sentry_sa = corev1.ServiceAccount {
    metadata = {
        name = "dapr-sentry"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr-sentry"
            "app.kubernetes.io/component" = "sentry"
        }
    }
}

# Dapr Placement ServiceAccount
_dapr_placement_sa = corev1.ServiceAccount {
    metadata = {
        name = "dapr-placement-server"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr-placement"
            "app.kubernetes.io/component" = "placement"
        }
    }
}

# Dapr Injector ServiceAccount
_dapr_injector_sa = corev1.ServiceAccount {
    metadata = {
        name = "dapr-injector"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr-injector"
            "app.kubernetes.io/component" = "injector"
        }
    }
}

# Dapr ClusterRole
_dapr_cluster_role = rbacv1.ClusterRole {
    metadata = {
        name = "dapr-operator"
        labels = {
            "app.kubernetes.io/name" = "dapr"
        }
    }
    rules = [
        # Dapr CRDs
        {
            apiGroups = ["dapr.io"]
            resources = ["components", "configurations", "subscriptions", "resiliencies", "httpendpoints"]
            verbs = ["*"]
        }
        # Core resources
        {
            apiGroups = [""]
            resources = ["pods", "services", "endpoints", "secrets", "configmaps", "namespaces", "serviceaccounts"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        # Apps resources
        {
            apiGroups = ["apps"]
            resources = ["deployments", "statefulsets", "daemonsets", "replicasets"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        # Events
        {
            apiGroups = [""]
            resources = ["events"]
            verbs = ["create", "patch"]
        }
        # Coordination for leader election
        {
            apiGroups = ["coordination.k8s.io"]
            resources = ["leases"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        # Admission webhooks
        {
            apiGroups = ["admissionregistration.k8s.io"]
            resources = ["mutatingwebhookconfigurations", "validatingwebhookconfigurations"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        # API services for sidecar
        {
            apiGroups = ["apiregistration.k8s.io"]
            resources = ["apiservices"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
    ]
}

# Dapr ClusterRoleBindings
_dapr_operator_crb = rbacv1.ClusterRoleBinding {
    metadata = {
        name = "dapr-operator"
        labels = {
            "app.kubernetes.io/name" = "dapr-operator"
        }
    }
    roleRef = {
        apiGroup = "rbac.authorization.k8s.io"
        kind = "ClusterRole"
        name = "dapr-operator"
    }
    subjects = [
        {
            kind = "ServiceAccount"
            name = "dapr-operator"
            namespace = NAMESPACE
        }
    ]
}

_dapr_injector_crb = rbacv1.ClusterRoleBinding {
    metadata = {
        name = "dapr-injector"
        labels = {
            "app.kubernetes.io/name" = "dapr-injector"
        }
    }
    roleRef = {
        apiGroup = "rbac.authorization.k8s.io"
        kind = "ClusterRole"
        name = "dapr-operator"
    }
    subjects = [
        {
            kind = "ServiceAccount"
            name = "dapr-injector"
            namespace = NAMESPACE
        }
    ]
}

# Dapr Sentry Deployment (Certificate Authority)
_dapr_sentry_deployment = appsv1.Deployment {
    metadata = {
        name = "dapr-sentry"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr-sentry"
            "app.kubernetes.io/component" = "sentry"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "dapr-sentry"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "dapr-sentry"
                "app.kubernetes.io/component" = "sentry"
            }
            spec = {
                serviceAccountName = "dapr-sentry"
                containers = [
                    {
                        name = "dapr-sentry"
                        image = DAPR_SENTRY_IMAGE
                        args = [
                            "--log-level=info"
                            "--enable-metrics=true"
                            "--metrics-port=9090"
                            "--trust-domain=cluster.local"
                        ]
                        ports = [
                            {
                                name = "grpc"
                                containerPort = 50001
                                protocol = "TCP"
                            }
                            {
                                name = "metrics"
                                containerPort = 9090
                                protocol = "TCP"
                            }
                        ]
                        env = [
                            {
                                name = "NAMESPACE"
                                valueFrom.fieldRef.fieldPath = "metadata.namespace"
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "100m"
                                memory = "100Mi"
                            }
                            limits = {
                                cpu = "500m"
                                memory = "500Mi"
                            }
                        }
                        livenessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = 8080
                            }
                            initialDelaySeconds = 3
                            periodSeconds = 3
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = 8080
                            }
                            initialDelaySeconds = 3
                            periodSeconds = 3
                        }
                        securityContext = {
                            allowPrivilegeEscalation = False
                            readOnlyRootFilesystem = True
                            runAsNonRoot = True
                            capabilities.drop = ["ALL"]
                        }
                        volumeMounts = [
                            {
                                name = "credentials"
                                mountPath = "/var/run/secrets/dapr.io/credentials"
                                readOnly = True
                            }
                        ]
                    }
                ]
                volumes = [
                    {
                        name = "credentials"
                        secret = {
                            secretName = "dapr-trust-bundle"
                            optional = True
                        }
                    }
                ]
            }
        }
    }
}

# Dapr Operator Deployment
_dapr_operator_deployment = appsv1.Deployment {
    metadata = {
        name = "dapr-operator"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr-operator"
            "app.kubernetes.io/component" = "operator"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "dapr-operator"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "dapr-operator"
                "app.kubernetes.io/component" = "operator"
            }
            spec = {
                serviceAccountName = "dapr-operator"
                containers = [
                    {
                        name = "dapr-operator"
                        image = DAPR_OPERATOR_IMAGE
                        command = ["/operator"]
                        args = [
                            "--log-level=info"
                            "--enable-metrics=true"
                            "--metrics-port=9090"
                            "--watch-interval=0"
                        ]
                        ports = [
                            {
                                name = "https"
                                containerPort = 6500
                                protocol = "TCP"
                            }
                            {
                                name = "metrics"
                                containerPort = 9090
                                protocol = "TCP"
                            }
                        ]
                        env = [
                            {
                                name = "NAMESPACE"
                                valueFrom.fieldRef.fieldPath = "metadata.namespace"
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "100m"
                                memory = "100Mi"
                            }
                            limits = {
                                cpu = "500m"
                                memory = "500Mi"
                            }
                        }
                        livenessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = 8080
                            }
                            initialDelaySeconds = 3
                            periodSeconds = 3
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = 8080
                            }
                            initialDelaySeconds = 3
                            periodSeconds = 3
                        }
                        securityContext = {
                            allowPrivilegeEscalation = False
                            readOnlyRootFilesystem = True
                            runAsNonRoot = True
                            capabilities.drop = ["ALL"]
                        }
                        volumeMounts = [
                            {
                                name = "credentials"
                                mountPath = "/var/run/secrets/dapr.io/credentials"
                                readOnly = True
                            }
                        ]
                    }
                ]
                volumes = [
                    {
                        name = "credentials"
                        secret = {
                            secretName = "dapr-trust-bundle"
                            optional = True
                        }
                    }
                ]
            }
        }
    }
}

# Dapr Placement Deployment (Actor placement)
_dapr_placement_deployment = appsv1.StatefulSet {
    metadata = {
        name = "dapr-placement-server"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr-placement"
            "app.kubernetes.io/component" = "placement"
        }
    }
    spec = {
        serviceName = "dapr-placement-server"
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "dapr-placement"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "dapr-placement"
                "app.kubernetes.io/component" = "placement"
            }
            spec = {
                serviceAccountName = "dapr-placement-server"
                containers = [
                    {
                        name = "dapr-placement"
                        image = DAPR_PLACEMENT_IMAGE
                        args = [
                            "--log-level=info"
                            "--enable-metrics=true"
                            "--metrics-port=9090"
                            "--replicationFactor=100"
                        ]
                        ports = [
                            {
                                name = "api"
                                containerPort = 50005
                                protocol = "TCP"
                            }
                            {
                                name = "raft"
                                containerPort = 8201
                                protocol = "TCP"
                            }
                            {
                                name = "metrics"
                                containerPort = 9090
                                protocol = "TCP"
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "100m"
                                memory = "100Mi"
                            }
                            limits = {
                                cpu = "500m"
                                memory = "500Mi"
                            }
                        }
                        livenessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = 8080
                            }
                            initialDelaySeconds = 3
                            periodSeconds = 3
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = 8080
                            }
                            initialDelaySeconds = 3
                            periodSeconds = 3
                        }
                        securityContext = {
                            allowPrivilegeEscalation = False
                            runAsNonRoot = True
                            capabilities.drop = ["ALL"]
                        }
                    }
                ]
            }
        }
    }
}

# Dapr Sidecar Injector Deployment
_dapr_injector_deployment = appsv1.Deployment {
    metadata = {
        name = "dapr-sidecar-injector"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr-injector"
            "app.kubernetes.io/component" = "injector"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "dapr-injector"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "dapr-injector"
                "app.kubernetes.io/component" = "injector"
            }
            spec = {
                serviceAccountName = "dapr-injector"
                containers = [
                    {
                        name = "dapr-sidecar-injector"
                        image = DAPR_OPERATOR_IMAGE
                        command = ["/injector"]
                        args = [
                            "--log-level=info"
                            "--enable-metrics=true"
                            "--metrics-port=9090"
                            "--sidecar-image=${DAPR_SIDECAR_IMAGE}"
                            "--sidecar-image-pull-policy=IfNotPresent"
                        ]
                        ports = [
                            {
                                name = "https"
                                containerPort = 4000
                                protocol = "TCP"
                            }
                            {
                                name = "metrics"
                                containerPort = 9090
                                protocol = "TCP"
                            }
                        ]
                        env = [
                            {
                                name = "NAMESPACE"
                                valueFrom.fieldRef.fieldPath = "metadata.namespace"
                            }
                            {
                                name = "SIDECAR_IMAGE"
                                value = DAPR_SIDECAR_IMAGE
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "100m"
                                memory = "100Mi"
                            }
                            limits = {
                                cpu = "500m"
                                memory = "500Mi"
                            }
                        }
                        livenessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = 8080
                            }
                            initialDelaySeconds = 3
                            periodSeconds = 3
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = 8080
                            }
                            initialDelaySeconds = 3
                            periodSeconds = 3
                        }
                        securityContext = {
                            allowPrivilegeEscalation = False
                            readOnlyRootFilesystem = True
                            runAsNonRoot = True
                            capabilities.drop = ["ALL"]
                        }
                        volumeMounts = [
                            {
                                name = "cert"
                                mountPath = "/var/run/secrets/dapr.io/tls"
                                readOnly = True
                            }
                        ]
                    }
                ]
                volumes = [
                    {
                        name = "cert"
                        secret = {
                            secretName = "dapr-sidecar-injector-cert"
                            optional = True
                        }
                    }
                ]
            }
        }
    }
}

# Services
_dapr_sentry_service = corev1.Service {
    metadata = {
        name = "dapr-sentry"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr-sentry"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "dapr-sentry"
        }
        ports = [
            {
                name = "grpc"
                port = 80
                targetPort = 50001
                protocol = "TCP"
            }
            {
                name = "metrics"
                port = 9090
                targetPort = 9090
                protocol = "TCP"
            }
        ]
    }
}

_dapr_api_service = corev1.Service {
    metadata = {
        name = "dapr-api"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr-operator"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "dapr-operator"
        }
        ports = [
            {
                name = "https"
                port = 443
                targetPort = 6500
                protocol = "TCP"
            }
        ]
    }
}

_dapr_placement_service = corev1.Service {
    metadata = {
        name = "dapr-placement-server"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr-placement"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "dapr-placement"
        }
        ports = [
            {
                name = "api"
                port = 50005
                targetPort = 50005
                protocol = "TCP"
            }
            {
                name = "raft"
                port = 8201
                targetPort = 8201
                protocol = "TCP"
            }
        ]
    }
}

_dapr_injector_service = corev1.Service {
    metadata = {
        name = "dapr-sidecar-injector"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "dapr-injector"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "dapr-injector"
        }
        ports = [
            {
                name = "https"
                port = 443
                targetPort = 4000
                protocol = "TCP"
            }
        ]
    }
}

# Dapr Configuration CRD schema
schema DaprConfiguration:
    """Dapr Configuration CRD"""
    apiVersion: str = "dapr.io/v1alpha1"
    kind: str = "Configuration"
    metadata: {str:any}
    spec: {str:any}

# Default Dapr Configuration with tracing to Jaeger
_dapr_default_config = DaprConfiguration {
    metadata = {
        name = "daprsystem"
        namespace = NAMESPACE
    }
    spec = {
        tracing = {
            samplingRate = "1"
            otel = {
                endpointAddress = "otel-collector.monitoring:4317"
                isSecure = False
                protocol = "grpc"
            }
        }
        metric = {
            enabled = True
        }
        mtls = {
            enabled = True
            workloadCertTTL = "24h"
            allowedClockSkew = "15m"
        }
        logging = {
            apiLogging = {
                enabled = True
                obfuscateURLs = True
            }
        }
    }
}

# Dapr Component CRD schema
schema DaprComponent:
    """Dapr Component CRD"""
    apiVersion: str = "dapr.io/v1alpha1"
    kind: str = "Component"
    metadata: {str:any}
    spec: {str:any}

# Example: State store with Redis
_example_statestore = DaprComponent {
    metadata = {
        name = "statestore"
        namespace = NAMESPACE
    }
    spec = {
        $type = "state.redis"
        version = "v1"
        metadata = [
            {name = "redisHost", value = "redis.data:6379"}
            {name = "redisPassword", value = ""}
            {name = "actorStateStore", value = "true"}
        ]
    }
}

# Example: Pub/sub with Redis
_example_pubsub = DaprComponent {
    metadata = {
        name = "pubsub"
        namespace = NAMESPACE
    }
    spec = {
        $type = "pubsub.redis"
        version = "v1"
        metadata = [
            {name = "redisHost", value = "redis.data:6379"}
            {name = "redisPassword", value = ""}
        ]
    }
}

dapr_items = [
    _dapr_namespace
    _dapr_operator_sa
    _dapr_sentry_sa
    _dapr_placement_sa
    _dapr_injector_sa
    _dapr_cluster_role
    _dapr_operator_crb
    _dapr_injector_crb
    _dapr_sentry_deployment
    _dapr_operator_deployment
    _dapr_placement_deployment
    _dapr_injector_deployment
    _dapr_sentry_service
    _dapr_api_service
    _dapr_placement_service
    _dapr_injector_service
    _dapr_default_config
]

# Example components (optional)
dapr_example_components = [
    _example_statestore
    _example_pubsub
]
