# FluxCD - CNCF Graduated Project
# GitOps Continuous Delivery for Kubernetes
# https://fluxcd.io/

import k8s.api.core.v1 as corev1
import k8s.api.apps.v1 as appsv1
import k8s.api.rbac.v1 as rbacv1
import k8s.api.networking.v1 as networkingv1

FLUX_VERSION = "2.3.0"
SOURCE_CONTROLLER_IMAGE = "ghcr.io/fluxcd/source-controller:v1.3.0"
KUSTOMIZE_CONTROLLER_IMAGE = "ghcr.io/fluxcd/kustomize-controller:v1.3.0"
HELM_CONTROLLER_IMAGE = "ghcr.io/fluxcd/helm-controller:v1.0.1"
NOTIFICATION_CONTROLLER_IMAGE = "ghcr.io/fluxcd/notification-controller:v1.3.0"
NAMESPACE = "flux-system"

# Flux Namespace
_flux_namespace = corev1.Namespace {
    metadata = {
        name = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "flux"
            "app.kubernetes.io/instance" = "flux-system"
            "app.kubernetes.io/managed-by" = "kcl"
            "cncf.io/project" = "graduated"
        }
    }
}

# Source Controller ServiceAccount
_source_controller_sa = corev1.ServiceAccount {
    metadata = {
        name = "source-controller"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "source-controller"
            "app.kubernetes.io/component" = "source"
        }
    }
}

# Kustomize Controller ServiceAccount
_kustomize_controller_sa = corev1.ServiceAccount {
    metadata = {
        name = "kustomize-controller"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "kustomize-controller"
            "app.kubernetes.io/component" = "kustomize"
        }
    }
}

# Helm Controller ServiceAccount
_helm_controller_sa = corev1.ServiceAccount {
    metadata = {
        name = "helm-controller"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "helm-controller"
            "app.kubernetes.io/component" = "helm"
        }
    }
}

# Notification Controller ServiceAccount
_notification_controller_sa = corev1.ServiceAccount {
    metadata = {
        name = "notification-controller"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "notification-controller"
            "app.kubernetes.io/component" = "notification"
        }
    }
}

# Flux ClusterRole (combined for all controllers)
_flux_cluster_role = rbacv1.ClusterRole {
    metadata = {
        name = "flux-controllers"
        labels = {
            "app.kubernetes.io/name" = "flux"
        }
    }
    rules = [
        # Source controller permissions
        {
            apiGroups = ["source.toolkit.fluxcd.io"]
            resources = ["*"]
            verbs = ["*"]
        }
        # Kustomize controller permissions
        {
            apiGroups = ["kustomize.toolkit.fluxcd.io"]
            resources = ["*"]
            verbs = ["*"]
        }
        # Helm controller permissions
        {
            apiGroups = ["helm.toolkit.fluxcd.io"]
            resources = ["*"]
            verbs = ["*"]
        }
        # Notification controller permissions
        {
            apiGroups = ["notification.toolkit.fluxcd.io"]
            resources = ["*"]
            verbs = ["*"]
        }
        # Image automation permissions
        {
            apiGroups = ["image.toolkit.fluxcd.io"]
            resources = ["*"]
            verbs = ["*"]
        }
        # Core resources
        {
            apiGroups = [""]
            resources = ["configmaps", "secrets", "serviceaccounts", "events", "namespaces"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        {
            apiGroups = [""]
            resources = ["services"]
            verbs = ["get", "list", "watch"]
        }
        # Apps resources for deployments
        {
            apiGroups = ["apps"]
            resources = ["deployments", "replicasets", "statefulsets", "daemonsets"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        # Batch resources
        {
            apiGroups = ["batch"]
            resources = ["jobs", "cronjobs"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        # RBAC resources
        {
            apiGroups = ["rbac.authorization.k8s.io"]
            resources = ["clusterroles", "clusterrolebindings", "roles", "rolebindings"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        # Networking
        {
            apiGroups = ["networking.k8s.io"]
            resources = ["ingresses", "networkpolicies"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        # For Helm releases
        {
            apiGroups = ["*"]
            resources = ["*"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
        # Leader election
        {
            apiGroups = ["coordination.k8s.io"]
            resources = ["leases"]
            verbs = ["get", "list", "watch", "create", "update", "patch", "delete"]
        }
    ]
}

# Flux ClusterRoleBindings
_source_controller_crb = rbacv1.ClusterRoleBinding {
    metadata = {
        name = "source-controller"
        labels = {
            "app.kubernetes.io/name" = "source-controller"
        }
    }
    roleRef = {
        apiGroup = "rbac.authorization.k8s.io"
        kind = "ClusterRole"
        name = "flux-controllers"
    }
    subjects = [
        {
            kind = "ServiceAccount"
            name = "source-controller"
            namespace = NAMESPACE
        }
    ]
}

_kustomize_controller_crb = rbacv1.ClusterRoleBinding {
    metadata = {
        name = "kustomize-controller"
        labels = {
            "app.kubernetes.io/name" = "kustomize-controller"
        }
    }
    roleRef = {
        apiGroup = "rbac.authorization.k8s.io"
        kind = "ClusterRole"
        name = "flux-controllers"
    }
    subjects = [
        {
            kind = "ServiceAccount"
            name = "kustomize-controller"
            namespace = NAMESPACE
        }
    ]
}

_helm_controller_crb = rbacv1.ClusterRoleBinding {
    metadata = {
        name = "helm-controller"
        labels = {
            "app.kubernetes.io/name" = "helm-controller"
        }
    }
    roleRef = {
        apiGroup = "rbac.authorization.k8s.io"
        kind = "ClusterRole"
        name = "flux-controllers"
    }
    subjects = [
        {
            kind = "ServiceAccount"
            name = "helm-controller"
            namespace = NAMESPACE
        }
    ]
}

_notification_controller_crb = rbacv1.ClusterRoleBinding {
    metadata = {
        name = "notification-controller"
        labels = {
            "app.kubernetes.io/name" = "notification-controller"
        }
    }
    roleRef = {
        apiGroup = "rbac.authorization.k8s.io"
        kind = "ClusterRole"
        name = "flux-controllers"
    }
    subjects = [
        {
            kind = "ServiceAccount"
            name = "notification-controller"
            namespace = NAMESPACE
        }
    ]
}

# Source Controller Deployment
_source_controller_deployment = appsv1.Deployment {
    metadata = {
        name = "source-controller"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "source-controller"
            "app.kubernetes.io/component" = "source"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "source-controller"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "source-controller"
            }
            spec = {
                serviceAccountName = "source-controller"
                securityContext = {
                    runAsNonRoot = True
                    fsGroup = 1337
                }
                containers = [
                    {
                        name = "manager"
                        image = SOURCE_CONTROLLER_IMAGE
                        args = [
                            "--events-addr=http://notification-controller.flux-system.svc.cluster.local./"
                            "--watch-all-namespaces=true"
                            "--log-level=info"
                            "--log-encoding=json"
                            "--storage-path=/data"
                            "--storage-adv-addr=source-controller.flux-system.svc.cluster.local."
                        ]
                        ports = [
                            {
                                name = "http"
                                containerPort = 9090
                                protocol = "TCP"
                            }
                            {
                                name = "http-prom"
                                containerPort = 8080
                                protocol = "TCP"
                            }
                            {
                                name = "healthz"
                                containerPort = 9440
                                protocol = "TCP"
                            }
                        ]
                        env = [
                            {
                                name = "RUNTIME_NAMESPACE"
                                valueFrom.fieldRef.fieldPath = "metadata.namespace"
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "50m"
                                memory = "64Mi"
                            }
                            limits = {
                                cpu = "1000m"
                                memory = "1Gi"
                            }
                        }
                        volumeMounts = [
                            {
                                name = "data"
                                mountPath = "/data"
                            }
                            {
                                name = "tmp"
                                mountPath = "/tmp"
                            }
                        ]
                        livenessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = "healthz"
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 10
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = "healthz"
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 10
                        }
                        securityContext = {
                            allowPrivilegeEscalation = False
                            readOnlyRootFilesystem = True
                            runAsNonRoot = True
                            capabilities.drop = ["ALL"]
                            seccompProfile = {
                                $type = "RuntimeDefault"
                            }
                        }
                    }
                ]
                volumes = [
                    {
                        name = "data"
                        emptyDir = {}
                    }
                    {
                        name = "tmp"
                        emptyDir = {}
                    }
                ]
            }
        }
    }
}

# Kustomize Controller Deployment
_kustomize_controller_deployment = appsv1.Deployment {
    metadata = {
        name = "kustomize-controller"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "kustomize-controller"
            "app.kubernetes.io/component" = "kustomize"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "kustomize-controller"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "kustomize-controller"
            }
            spec = {
                serviceAccountName = "kustomize-controller"
                securityContext = {
                    runAsNonRoot = True
                    fsGroup = 1337
                }
                containers = [
                    {
                        name = "manager"
                        image = KUSTOMIZE_CONTROLLER_IMAGE
                        args = [
                            "--events-addr=http://notification-controller.flux-system.svc.cluster.local./"
                            "--watch-all-namespaces=true"
                            "--log-level=info"
                            "--log-encoding=json"
                        ]
                        ports = [
                            {
                                name = "http-prom"
                                containerPort = 8080
                                protocol = "TCP"
                            }
                            {
                                name = "healthz"
                                containerPort = 9440
                                protocol = "TCP"
                            }
                        ]
                        env = [
                            {
                                name = "RUNTIME_NAMESPACE"
                                valueFrom.fieldRef.fieldPath = "metadata.namespace"
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "100m"
                                memory = "64Mi"
                            }
                            limits = {
                                cpu = "1000m"
                                memory = "1Gi"
                            }
                        }
                        volumeMounts = [
                            {
                                name = "tmp"
                                mountPath = "/tmp"
                            }
                        ]
                        livenessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = "healthz"
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 10
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/readyz"
                                port = "healthz"
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 10
                        }
                        securityContext = {
                            allowPrivilegeEscalation = False
                            readOnlyRootFilesystem = True
                            runAsNonRoot = True
                            capabilities.drop = ["ALL"]
                            seccompProfile = {
                                $type = "RuntimeDefault"
                            }
                        }
                    }
                ]
                volumes = [
                    {
                        name = "tmp"
                        emptyDir = {}
                    }
                ]
            }
        }
    }
}

# Helm Controller Deployment
_helm_controller_deployment = appsv1.Deployment {
    metadata = {
        name = "helm-controller"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "helm-controller"
            "app.kubernetes.io/component" = "helm"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "helm-controller"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "helm-controller"
            }
            spec = {
                serviceAccountName = "helm-controller"
                securityContext = {
                    runAsNonRoot = True
                    fsGroup = 1337
                }
                containers = [
                    {
                        name = "manager"
                        image = HELM_CONTROLLER_IMAGE
                        args = [
                            "--events-addr=http://notification-controller.flux-system.svc.cluster.local./"
                            "--watch-all-namespaces=true"
                            "--log-level=info"
                            "--log-encoding=json"
                        ]
                        ports = [
                            {
                                name = "http-prom"
                                containerPort = 8080
                                protocol = "TCP"
                            }
                            {
                                name = "healthz"
                                containerPort = 9440
                                protocol = "TCP"
                            }
                        ]
                        env = [
                            {
                                name = "RUNTIME_NAMESPACE"
                                valueFrom.fieldRef.fieldPath = "metadata.namespace"
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "100m"
                                memory = "64Mi"
                            }
                            limits = {
                                cpu = "1000m"
                                memory = "1Gi"
                            }
                        }
                        volumeMounts = [
                            {
                                name = "tmp"
                                mountPath = "/tmp"
                            }
                        ]
                        livenessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = "healthz"
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 10
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/readyz"
                                port = "healthz"
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 10
                        }
                        securityContext = {
                            allowPrivilegeEscalation = False
                            readOnlyRootFilesystem = True
                            runAsNonRoot = True
                            capabilities.drop = ["ALL"]
                            seccompProfile = {
                                $type = "RuntimeDefault"
                            }
                        }
                    }
                ]
                volumes = [
                    {
                        name = "tmp"
                        emptyDir = {}
                    }
                ]
            }
        }
    }
}

# Notification Controller Deployment
_notification_controller_deployment = appsv1.Deployment {
    metadata = {
        name = "notification-controller"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "notification-controller"
            "app.kubernetes.io/component" = "notification"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "notification-controller"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "notification-controller"
            }
            spec = {
                serviceAccountName = "notification-controller"
                securityContext = {
                    runAsNonRoot = True
                    fsGroup = 1337
                }
                containers = [
                    {
                        name = "manager"
                        image = NOTIFICATION_CONTROLLER_IMAGE
                        args = [
                            "--watch-all-namespaces=true"
                            "--log-level=info"
                            "--log-encoding=json"
                        ]
                        ports = [
                            {
                                name = "http"
                                containerPort = 9090
                                protocol = "TCP"
                            }
                            {
                                name = "http-prom"
                                containerPort = 8080
                                protocol = "TCP"
                            }
                            {
                                name = "http-webhook"
                                containerPort = 9292
                                protocol = "TCP"
                            }
                            {
                                name = "healthz"
                                containerPort = 9440
                                protocol = "TCP"
                            }
                        ]
                        env = [
                            {
                                name = "RUNTIME_NAMESPACE"
                                valueFrom.fieldRef.fieldPath = "metadata.namespace"
                            }
                        ]
                        resources = {
                            requests = {
                                cpu = "50m"
                                memory = "64Mi"
                            }
                            limits = {
                                cpu = "500m"
                                memory = "512Mi"
                            }
                        }
                        volumeMounts = [
                            {
                                name = "tmp"
                                mountPath = "/tmp"
                            }
                        ]
                        livenessProbe = {
                            httpGet = {
                                path = "/healthz"
                                port = "healthz"
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 10
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/readyz"
                                port = "healthz"
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 10
                        }
                        securityContext = {
                            allowPrivilegeEscalation = False
                            readOnlyRootFilesystem = True
                            runAsNonRoot = True
                            capabilities.drop = ["ALL"]
                            seccompProfile = {
                                $type = "RuntimeDefault"
                            }
                        }
                    }
                ]
                volumes = [
                    {
                        name = "tmp"
                        emptyDir = {}
                    }
                ]
            }
        }
    }
}

# Services
_source_controller_service = corev1.Service {
    metadata = {
        name = "source-controller"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "source-controller"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "source-controller"
        }
        ports = [
            {
                name = "http"
                port = 80
                targetPort = 9090
                protocol = "TCP"
            }
        ]
    }
}

_notification_controller_service = corev1.Service {
    metadata = {
        name = "notification-controller"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "notification-controller"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "notification-controller"
        }
        ports = [
            {
                name = "http"
                port = 80
                targetPort = 9090
                protocol = "TCP"
            }
            {
                name = "http-webhook"
                port = 9292
                targetPort = 9292
                protocol = "TCP"
            }
        ]
    }
}

# Webhook receiver service for external triggers
_webhook_receiver_service = corev1.Service {
    metadata = {
        name = "webhook-receiver"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "webhook-receiver"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "notification-controller"
        }
        ports = [
            {
                name = "http"
                port = 80
                targetPort = 9292
                protocol = "TCP"
            }
        ]
    }
}

# Network Policy to allow Flux controllers to communicate
_flux_network_policy = networkingv1.NetworkPolicy {
    metadata = {
        name = "allow-flux-controllers"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "flux"
        }
    }
    spec = {
        podSelector = {}
        policyTypes = ["Ingress", "Egress"]
        ingress = [
            {
                from = [
                    {
                        namespaceSelector = {
                            matchLabels = {
                                "kubernetes.io/metadata.name" = NAMESPACE
                            }
                        }
                    }
                ]
            }
        ]
        egress = [
            {}
        ]
    }
}

# Flux GitRepository CRD schema (for reference)
schema GitRepository:
    """Flux GitRepository source"""
    apiVersion: str = "source.toolkit.fluxcd.io/v1"
    kind: str = "GitRepository"
    metadata: {str:any}
    spec: {str:any}

# Flux Kustomization CRD schema (for reference)
schema Kustomization:
    """Flux Kustomization"""
    apiVersion: str = "kustomize.toolkit.fluxcd.io/v1"
    kind: str = "Kustomization"
    metadata: {str:any}
    spec: {str:any}

# Flux HelmRelease CRD schema (for reference)
schema HelmRelease:
    """Flux HelmRelease"""
    apiVersion: str = "helm.toolkit.fluxcd.io/v2"
    kind: str = "HelmRelease"
    metadata: {str:any}
    spec: {str:any}

fluxcd_items = [
    _flux_namespace
    _source_controller_sa
    _kustomize_controller_sa
    _helm_controller_sa
    _notification_controller_sa
    _flux_cluster_role
    _source_controller_crb
    _kustomize_controller_crb
    _helm_controller_crb
    _notification_controller_crb
    _source_controller_deployment
    _kustomize_controller_deployment
    _helm_controller_deployment
    _notification_controller_deployment
    _source_controller_service
    _notification_controller_service
    _webhook_receiver_service
    _flux_network_policy
]
