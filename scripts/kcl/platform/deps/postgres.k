# PostgreSQL Dependency Module
# Supports: CNPG (internal), Neon, Supabase, Aiven, AWS RDS, GCP CloudSQL

import k8s.api.core.v1 as corev1
import .base as base

# PostgreSQL-specific configuration
schema PostgresConfig(base.DependencyConfig):
    """PostgreSQL database configuration"""
    database: str = "app"                 # Database name
    username: str = "app"                 # Username
    storage_size: str = "10Gi"            # Storage size for internal
    instances: int = 1                    # Number of replicas

    # Connection pooling
    pooler_enabled: bool = False
    pooler_mode: str = "transaction"      # session, transaction, statement
    pooler_instances: int = 1

# Internal CNPG cluster
schema CNPGCluster:
    apiVersion: str = "postgresql.cnpg.io/v1"
    kind: str = "Cluster"
    metadata: {str: any}
    spec: {str: any}

# Crossplane PostgreSQL claim (for external providers)
schema PostgreSQLClaim:
    apiVersion: str = "database.yurikrupnik.com/v1alpha1"
    kind: str = "PostgreSQLInstance"
    metadata: {str: any}
    spec: {str: any}

# Create internal CNPG postgres
_create_internal_postgres = lambda name: str, namespace: str, config: PostgresConfig -> base.DependencyOutput {
    _cluster_name = "${name}-postgres"
    _secret_name = "${_cluster_name}-app"

    _cluster = CNPGCluster {
        metadata: {
            name: _cluster_name
            namespace: namespace
            labels: {
                "app.kubernetes.io/name": _cluster_name
                "app.kubernetes.io/component": "database"
                "platform.yurikrupnik.com/postgres": "true"
                "platform.yurikrupnik.com/internal": "true"
            }
        }
        spec: {
            instances: config.instances
            storage.size: config.storage_size

            bootstrap.initdb: {
                database: config.database
                owner: config.username
            }

            postgresql: {
                parameters: {
                    max_connections: "200"
                    shared_buffers: "256MB"
                }
            }

            monitoring.enablePodMonitor: True

            # Enable connection pooling if requested
            if config.pooler_enabled:
                pooler: {
                    instances: config.pooler_instances
                    type: "rw"
                    pgbouncer: {
                        poolMode: config.pooler_mode
                        parameters: {
                            max_client_conn: "1000"
                            default_pool_size: "20"
                        }
                    }
                }
        }
    }

    _pooler_host = "${_cluster_name}-pooler.${namespace}.svc" if config.pooler_enabled else "${_cluster_name}-rw.${namespace}.svc"

    base.DependencyOutput {
        name: "postgres"
        env_vars: [
            base.create_env_from_secret("POSTGRES_USER", _secret_name, "username")
            base.create_env_from_secret("POSTGRES_PASSWORD", _secret_name, "password")
            base.create_env_from_secret("POSTGRES_HOST", _secret_name, "host")
            base.create_env("POSTGRES_PORT", "5432")
            base.create_env("POSTGRES_DB", config.database)
            # Composite URL
            corev1.EnvVar {
                name: "DATABASE_URL"
                value: "postgres://$(POSTGRES_USER):$(POSTGRES_PASSWORD)@${_pooler_host}:5432/${config.database}"
            }
        ]
        resources: [_cluster]
        secrets: []
        labels: {
            "platform.yurikrupnik.com/postgres": "true"
            "platform.yurikrupnik.com/postgres-internal": "true"
            "platform.yurikrupnik.com/postgres-provider": "cnpg"
        }
        wait_for: ["${_cluster_name}"]
    }
}

# Create external Crossplane-managed postgres
_create_external_postgres = lambda name: str, namespace: str, config: PostgresConfig -> base.DependencyOutput {
    _claim_name = "${name}-postgres"
    _secret_name = "${_claim_name}-conn"
    _provider = config.provider.name if config.provider else "neon"

    _claim = PostgreSQLClaim {
        metadata: {
            name: _claim_name
            namespace: namespace
            labels: {
                "app.kubernetes.io/name": _claim_name
                "app.kubernetes.io/component": "database"
                "platform.yurikrupnik.com/postgres": "true"
            }
        }
        spec: {
            compositionSelector.matchLabels: {
                provider: _provider
            }
            parameters: {
                database: config.database
                region: config.provider.region if config.provider?.region else "us-east-1"
                tier: config.provider.tier if config.provider?.tier else "free"
                storageGB: int(config.storage_size.removesuffix("Gi"))
            }
            writeConnectionSecretToRef: {
                name: _secret_name
            }
        }
    }

    base.DependencyOutput {
        name: "postgres"
        env_vars: [
            base.create_env_from_secret("DATABASE_URL", _secret_name, "url")
            base.create_env_from_secret("POSTGRES_HOST", _secret_name, "host")
            base.create_env_from_secret("POSTGRES_USER", _secret_name, "username")
            base.create_env_from_secret("POSTGRES_PASSWORD", _secret_name, "password")
            base.create_env("POSTGRES_DB", config.database)
        ]
        resources: [_claim]
        secrets: []
        labels: {
            "platform.yurikrupnik.com/postgres": "true"
            "platform.yurikrupnik.com/postgres-provider": _provider
        }
        wait_for: ["${_claim_name}"]
    }
}

# Main entry point - creates postgres based on configuration
create_postgres = lambda name: str, namespace: str, config: PostgresConfig -> base.DependencyOutput {
    _internal = config.provider?.internal or not config.provider
    _create_internal_postgres(name, namespace, config) if _internal else _create_external_postgres(name, namespace, config)
}

# Quick helpers for common patterns
create_postgres_neon = lambda name: str, namespace: str -> base.DependencyOutput {
    create_postgres(name, namespace, PostgresConfig {
        provider: base.ProviderConfig {
            name: "neon"
            tier: "free"
        }
    })
}

create_postgres_internal = lambda name: str, namespace: str -> base.DependencyOutput {
    create_postgres(name, namespace, PostgresConfig {
        provider: base.ProviderConfig {
            name: "cnpg"
            internal: True
        }
    })
}

create_postgres_supabase = lambda name: str, namespace: str -> base.DependencyOutput {
    create_postgres(name, namespace, PostgresConfig {
        provider: base.ProviderConfig {
            name: "supabase"
        }
    })
}
