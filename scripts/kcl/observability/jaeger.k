# Jaeger - CNCF Graduated Project
# Distributed Tracing System
# https://www.jaegertracing.io/

import k8s.api.core.v1 as corev1
import k8s.api.apps.v1 as appsv1

JAEGER_VERSION = "1.57"
JAEGER_IMAGE = "jaegertracing/all-in-one:${JAEGER_VERSION}"
NAMESPACE = "monitoring"

# Jaeger Deployment (all-in-one for simplicity)
_jaeger_deployment = appsv1.Deployment {
    metadata = {
        name = "jaeger"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "jaeger"
            "app.kubernetes.io/component" = "tracing"
            "cncf.io/project" = "graduated"
        }
    }
    spec = {
        replicas = 1
        selector.matchLabels = {
            "app.kubernetes.io/name" = "jaeger"
        }
        template = {
            metadata.labels = {
                "app.kubernetes.io/name" = "jaeger"
            }
            spec = {
                containers = [
                    {
                        name = "jaeger"
                        image = JAEGER_IMAGE
                        args = [
                            "--memory.max-traces=10000"
                            "--query.base-path=/jaeger"
                        ]
                        ports = [
                            {
                                name = "query-http"
                                containerPort = 16686
                                protocol = "TCP"
                            }
                            {
                                name = "query-grpc"
                                containerPort = 16685
                                protocol = "TCP"
                            }
                            {
                                name = "collector-grpc"
                                containerPort = 14250
                                protocol = "TCP"
                            }
                            {
                                name = "collector-http"
                                containerPort = 14268
                                protocol = "TCP"
                            }
                            {
                                name = "collector-zipkin"
                                containerPort = 9411
                                protocol = "TCP"
                            }
                            {
                                name = "admin"
                                containerPort = 14269
                                protocol = "TCP"
                            }
                            {
                                # OpenTelemetry Protocol (OTLP) gRPC
                                name = "otlp-grpc"
                                containerPort = 4317
                                protocol = "TCP"
                            }
                            {
                                # OpenTelemetry Protocol (OTLP) HTTP
                                name = "otlp-http"
                                containerPort = 4318
                                protocol = "TCP"
                            }
                        ]
                        env = [
                            {
                                name = "COLLECTOR_ZIPKIN_HOST_PORT"
                                value = ":9411"
                            }
                            {
                                name = "COLLECTOR_OTLP_ENABLED"
                                value = "true"
                            }
                            {
                                name = "SPAN_STORAGE_TYPE"
                                value = "memory"  # Use Elasticsearch/Cassandra in production
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
                        livenessProbe = {
                            httpGet = {
                                path = "/"
                                port = 14269
                            }
                            initialDelaySeconds = 10
                            periodSeconds = 10
                        }
                        readinessProbe = {
                            httpGet = {
                                path = "/"
                                port = 14269
                            }
                            initialDelaySeconds = 5
                            periodSeconds = 5
                        }
                    }
                ]
            }
        }
    }
}

# Jaeger Service - Query UI
_jaeger_query_service = corev1.Service {
    metadata = {
        name = "jaeger"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "jaeger"
            "app.kubernetes.io/component" = "query"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "jaeger"
        }
        ports = [
            {
                name = "query-http"
                port = 16686
                targetPort = 16686
                protocol = "TCP"
            }
            {
                name = "query-grpc"
                port = 16685
                targetPort = 16685
                protocol = "TCP"
            }
        ]
    }
}

# Jaeger Service - Collector (for receiving traces)
_jaeger_collector_service = corev1.Service {
    metadata = {
        name = "jaeger-collector"
        namespace = NAMESPACE
        labels = {
            "app.kubernetes.io/name" = "jaeger"
            "app.kubernetes.io/component" = "collector"
        }
    }
    spec = {
        selector = {
            "app.kubernetes.io/name" = "jaeger"
        }
        ports = [
            {
                name = "grpc"
                port = 14250
                targetPort = 14250
                protocol = "TCP"
            }
            {
                name = "http"
                port = 14268
                targetPort = 14268
                protocol = "TCP"
            }
            {
                name = "zipkin"
                port = 9411
                targetPort = 9411
                protocol = "TCP"
            }
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
        ]
    }
}

jaeger_items = [
    _jaeger_deployment
    _jaeger_query_service
    _jaeger_collector_service
]
