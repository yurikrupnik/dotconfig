# Redis Dependency Module
# Supports: Bitnami Redis (internal), Upstash, Redis Cloud, AWS ElastiCache

import k8s.api.core.v1 as corev1
import k8s.api.apps.v1 as appsv1
import .base as base

# Redis-specific configuration
schema RedisConfig(base.DependencyConfig):
    """Redis cache/queue configuration"""
    mode: str = "standalone"              # standalone, cluster, sentinel
    memory: str = "128Mi"                 # Memory limit
    persistence: bool = False             # Enable persistence
    password_enabled: bool = True

# Internal Redis (simple deployment for dev/staging)
schema RedisDeployment:
    deployment: appsv1.Deployment
    service: corev1.Service
    secret?: corev1.Secret

# Crossplane Redis claim
schema RedisClaim:
    apiVersion: str = "cache.yurikrupnik.com/v1alpha1"
    kind: str = "RedisInstance"
    metadata: {str: any}
    spec: {str: any}

# Create internal Redis
_create_internal_redis = lambda name: str, namespace: str, config: RedisConfig -> base.DependencyOutput {
    _redis_name = "${name}-redis"
    _secret_name = "${_redis_name}-auth"

    _password_secret = corev1.Secret {
        metadata: {
            name: _secret_name
            namespace: namespace
            labels: {
                "app.kubernetes.io/name": _redis_name
                "app.kubernetes.io/component": "cache"
            }
        }
        $type: "Opaque"
        stringData: {
            password: "$(RANDOM_PASSWORD)"  # Placeholder - operator generates
        }
    } if config.password_enabled else None

    _deployment = appsv1.Deployment {
        metadata: {
            name: _redis_name
            namespace: namespace
            labels: {
                "app.kubernetes.io/name": _redis_name
                "app.kubernetes.io/component": "cache"
                "platform.yurikrupnik.com/redis": "true"
                "platform.yurikrupnik.com/internal": "true"
            }
        }
        spec: {
            replicas: 1
            selector.matchLabels: {
                app: _redis_name
            }
            template.metadata.labels: {
                app: _redis_name
            }
            template.spec: {
                containers: [
                    {
                        name: "redis"
                        image: "redis:7-alpine"
                        ports: [{containerPort: 6379, name: "redis"}]
                        resources: {
                            requests: {memory: config.memory, cpu: "50m"}
                            limits: {memory: config.memory}
                        }
                        if config.password_enabled:
                            command: ["redis-server", "--requirepass", "$(REDIS_PASSWORD)"]
                            env: [
                                corev1.EnvVar {
                                    name: "REDIS_PASSWORD"
                                    valueFrom.secretKeyRef: {
                                        name: _secret_name
                                        key: "password"
                                    }
                                }
                            ]
                        if config.persistence:
                            volumeMounts: [{
                                name: "data"
                                mountPath: "/data"
                            }]
                    }
                ]
                if config.persistence:
                    volumes: [{
                        name: "data"
                        emptyDir: {}
                    }]
            }
        }
    }

    _service = corev1.Service {
        metadata: {
            name: _redis_name
            namespace: namespace
            labels: {
                "app.kubernetes.io/name": _redis_name
            }
        }
        spec: {
            selector: {app: _redis_name}
            ports: [{port: 6379, targetPort: "redis", name: "redis"}]
        }
    }

    _env_vars = [
        base.create_env("REDIS_HOST", "${_redis_name}.${namespace}.svc")
        base.create_env("REDIS_PORT", "6379")
    ]

    if config.password_enabled:
        _env_vars += [
            base.create_env_from_secret("REDIS_PASSWORD", _secret_name, "password")
            corev1.EnvVar {
                name: "REDIS_URL"
                value: "redis://:$(REDIS_PASSWORD)@${_redis_name}.${namespace}.svc:6379"
            }
        ]
    else:
        _env_vars += [
            base.create_env("REDIS_URL", "redis://${_redis_name}.${namespace}.svc:6379")
        ]

    _resources = [_deployment, _service]
    _secrets = [_password_secret] if _password_secret else []

    base.DependencyOutput {
        name: "redis"
        env_vars: _env_vars
        resources: _resources
        secrets: _secrets
        labels: {
            "platform.yurikrupnik.com/redis": "true"
            "platform.yurikrupnik.com/redis-internal": "true"
            "platform.yurikrupnik.com/redis-provider": "internal"
        }
        wait_for: ["deployment/${_redis_name}"]
    }
}

# Create external Crossplane-managed Redis
_create_external_redis = lambda name: str, namespace: str, config: RedisConfig -> base.DependencyOutput {
    _claim_name = "${name}-redis"
    _secret_name = "${_claim_name}-conn"
    _provider = config.provider.name if config.provider else "upstash"

    _claim = RedisClaim {
        metadata: {
            name: _claim_name
            namespace: namespace
            labels: {
                "app.kubernetes.io/name": _claim_name
                "app.kubernetes.io/component": "cache"
                "platform.yurikrupnik.com/redis": "true"
            }
        }
        spec: {
            compositionSelector.matchLabels: {
                provider: _provider
            }
            parameters: {
                region: config.provider.region if config.provider?.region else "us-east-1"
                tier: config.provider.tier if config.provider?.tier else "free"
                evictionPolicy: "volatile-lru"
            }
            writeConnectionSecretToRef: {
                name: _secret_name
            }
        }
    }

    base.DependencyOutput {
        name: "redis"
        env_vars: [
            base.create_env_from_secret("REDIS_URL", _secret_name, "url")
            base.create_env_from_secret("REDIS_HOST", _secret_name, "host")
            base.create_env_from_secret("REDIS_PORT", _secret_name, "port")
            base.create_env_from_secret("REDIS_PASSWORD", _secret_name, "password")
        ]
        resources: [_claim]
        secrets: []
        labels: {
            "platform.yurikrupnik.com/redis": "true"
            "platform.yurikrupnik.com/redis-provider": _provider
        }
        wait_for: ["${_claim_name}"]
    }
}

# Main entry point
create_redis = lambda name: str, namespace: str, config: RedisConfig -> base.DependencyOutput {
    _internal = config.provider?.internal or not config.provider
    _create_internal_redis(name, namespace, config) if _internal else _create_external_redis(name, namespace, config)
}

# Quick helpers
create_redis_internal = lambda name: str, namespace: str -> base.DependencyOutput {
    create_redis(name, namespace, RedisConfig {
        provider: base.ProviderConfig {
            name: "internal"
            internal: True
        }
    })
}

create_redis_upstash = lambda name: str, namespace: str -> base.DependencyOutput {
    create_redis(name, namespace, RedisConfig {
        provider: base.ProviderConfig {
            name: "upstash"
        }
    })
}
