# App Manifest Schema - The "package.json" for infrastructure dependencies
# Compose dependencies like lego blocks

import k8s.api.apps.v1 as appsv1
import k8s.api.core.v1 as corev1
import .base as base
import .postgres as pg
import .redis as rd

# Use k8s ResourceRequirements directly
# import k8s.api.core.v1.ResourceRequirements

# Monitoring configuration
schema MonitoringConfig:
    enabled: bool = True
    provider: str = "prometheus"          # prometheus, datadog, newrelic
    sentry_enabled: bool = False

# Auth configuration
schema AuthConfig:
    enabled: bool = False
    provider: str = "clerk"               # clerk, auth0, supabase, firebase
    credentials_secret?: str

# Queue/messaging configuration
schema QueueConfig:
    enabled: bool = False
    provider: str = "redis"               # redis, rabbitmq, kafka, sqs
    internal: bool = True

# Object storage configuration
schema StorageConfig:
    enabled: bool = False
    provider: str = "s3"                  # s3, r2, gcs, minio
    bucket_name?: str

# App Dependencies - declare what your app needs
schema AppDependencies:
    """Declare infrastructure dependencies for your application"""
    postgres?: pg.PostgresConfig
    redis?: rd.RedisConfig
    mongo?: base.DependencyConfig
    mysql?: base.DependencyConfig
    elasticsearch?: base.DependencyConfig

    # Auth providers
    auth?: AuthConfig

    # Messaging/queues
    queue?: QueueConfig

    # Storage
    storage?: StorageConfig

    # Monitoring
    monitoring?: MonitoringConfig

# App Manifest - the main configuration
schema AppManifest:
    """
    Infrastructure Package Manifest - like package.json for your app's infrastructure

    Example:
    ```
    app = AppManifest {
        name: "my-api"
        namespace: "production"
        image: "ghcr.io/myorg/my-api:v1.0.0"
        dependencies: {
            postgres: {
                provider: {name: "neon", tier: "free"}
                database: "myapp"
            }
            redis: {
                provider: {internal: True}
            }
            auth: {provider: "clerk"}
            monitoring: {enabled: True, sentry_enabled: True}
        }
    }
    ```
    """
    name: str
    namespace: str = "default"
    image: str
    replicas: int = 1
    port: int = 8080

    # Resource limits (uses k8s ResourceRequirements)
    resources?: corev1.ResourceRequirements

    # Dependencies
    dependencies: AppDependencies = AppDependencies {}

    # Additional environment variables
    extra_env?: [corev1.EnvVar]

    # Labels to add
    labels?: {str: str}

    # Annotations
    annotations?: {str: str}

# Resolve a single dependency type
_resolve_dependency = lambda name: str, namespace: str, dep_name: str, dep_config: any -> base.DependencyOutput {
    _result = base.DependencyOutput {
        name: dep_name
        env_vars: []
        resources: []
        secrets: []
        labels: {}
    }
    _result = pg.create_postgres(name, namespace, dep_config) if dep_name == "postgres" and dep_config else _result
    _result = rd.create_redis(name, namespace, dep_config) if dep_name == "redis" and dep_config else _result
    _result
}

# Resolve all dependencies from AppDependencies
resolve_dependencies = lambda name: str, namespace: str, deps: AppDependencies -> base.DependencyOutput {
    _outputs = []

    if deps.postgres:
        _outputs += [pg.create_postgres(name, namespace, deps.postgres)]

    if deps.redis:
        _outputs += [rd.create_redis(name, namespace, deps.redis)]

    # Add monitoring labels if enabled
    _monitoring_labels = {}
    if deps.monitoring?.enabled:
        _monitoring_labels = {
            "platform.yurikrupnik.com/monitoring": "true"
            "platform.yurikrupnik.com/monitoring-provider": deps.monitoring.provider
        }
        if deps.monitoring.sentry_enabled:
            _monitoring_labels["platform.yurikrupnik.com/sentry"] = "true"

    # Add auth labels if enabled
    _auth_labels = {}
    if deps.auth?.enabled:
        _auth_labels = {
            "platform.yurikrupnik.com/auth": "true"
            "platform.yurikrupnik.com/auth-provider": deps.auth.provider
        }

    # Merge all outputs
    _merged = base.merge_outputs(_outputs) if _outputs else base.DependencyOutput {
        name: "empty"
        env_vars: []
        resources: []
        secrets: []
        labels: {}
    }

    # Add extra labels
    base.DependencyOutput {
        name: _merged.name
        env_vars: _merged.env_vars
        resources: _merged.resources
        secrets: _merged.secrets
        labels: _merged.labels | _monitoring_labels | _auth_labels
        wait_for: _merged.wait_for
    }
}

# Generate complete application resources from manifest
generate_app_resources = lambda manifest: AppManifest -> [any] {
    # Resolve all dependencies
    _deps = resolve_dependencies(manifest.name, manifest.namespace, manifest.dependencies)

    # Combine app env vars with dependency env vars
    _all_env = _deps.env_vars + (manifest.extra_env or [])

    # Combine labels
    _all_labels = {
        "app.kubernetes.io/name": manifest.name
        "app.kubernetes.io/managed-by": "platform-operator"
        "platform.yurikrupnik.com/auto-label": "true"
    } | _deps.labels | (manifest.labels or {})

    # Create the deployment
    _deployment = appsv1.Deployment {
        metadata: {
            name: manifest.name
            namespace: manifest.namespace
            labels: _all_labels
            annotations: manifest.annotations or {}
        }
        spec: {
            replicas: manifest.replicas
            selector.matchLabels: {
                app: manifest.name
            }
            template.metadata: {
                labels: {app: manifest.name} | _all_labels
                annotations: {
                    "platform.yurikrupnik.com/auto-label": "true"
                } | (manifest.annotations or {})
            }
            template.spec.containers: [
                {
                    name: manifest.name
                    image: manifest.image
                    ports: [{containerPort: manifest.port, name: "http"}]
                    env: _all_env
                    resources: manifest.resources
                }
            ]
        }
    }

    # Create service
    _service = corev1.Service {
        metadata: {
            name: manifest.name
            namespace: manifest.namespace
            labels: _all_labels
        }
        spec: {
            selector: {app: manifest.name}
            ports: [{port: 80, targetPort: "http", name: "http"}]
        }
    }

    # Return all resources
    _deps.resources + _deps.secrets + [_deployment, _service]
}
