# Example App Manifests
# Shows how to compose infrastructure dependencies like lego blocks

import .app
import .app as appmod
import .postgres as pg
import .redis as rd
import .base as base

AppDependencies = appmod.AppDependencies

# Example 1: Simple API with internal postgres
simple_api = app.AppManifest {
    name: "simple-api"
    namespace: "default"
    image: "ghcr.io/myorg/simple-api:v1.0.0"
    replicas: 2
    dependencies: AppDependencies {
        postgres: pg.PostgresConfig {
            provider: base.ProviderConfig {
                name: "cnpg"
                internal: True
            }
            database: "api"
            storage_size: "5Gi"
        }
        #mongodb: mg.PostgresConfig {
        #    provider: base.ProviderConfig {
        #        name: "mongodb"
        #        internal: True
        #    }
        #    database: "api"
        #    storage_size: "5Gi"
        #}
    }
}

# Example 2: Production API with Neon Postgres + Upstash Redis
production_api = app.AppManifest {
    name: "prod-api"
    namespace: "production"
    image: "ghcr.io/myorg/api:v2.0.0"
    replicas: 3
    port: 3000
    resources: {
        requests: {cpu: "500m", memory: "512Mi"}
        limits: {memory: "1Gi"}
    }
    dependencies: {
        postgres: pg.PostgresConfig {
            provider: base.ProviderConfig {
                name: "neon"
                region: "us-east-1"
                tier: "standard"
            }
            database: "production"
            pooler_enabled: True
        }
        redis: rd.RedisConfig {
            provider: base.ProviderConfig {
                name: "upstash"
                region: "us-east-1"
            }
        }
        monitoring: {
            enabled: True
            provider: "datadog"
            sentry_enabled: True
        }
        auth: {
            enabled: True
            provider: "clerk"
        }
    }
    labels: {
        team: "backend"
        "cost-center": "engineering"
    }
}

# Example 3: Staging with internal everything (cost-effective)
staging_api = app.AppManifest {
    name: "staging-api"
    namespace: "staging"
    image: "ghcr.io/myorg/api:staging"
    replicas: 1
    dependencies: {
        postgres: pg.PostgresConfig {
            provider: base.ProviderConfig {
                name: "cnpg"
                internal: True
            }
            storage_size: "1Gi"
            instances: 1
        }
        redis: rd.RedisConfig {
            provider: base.ProviderConfig {
                name: "internal"
                internal: True
            }
            memory: "64Mi"
            persistence: False
        }
        monitoring: {
            enabled: True
            provider: "prometheus"
        }
    }
}

# Example 4: ML Workload with GPU and large storage
ml_worker = app.AppManifest {
    name: "ml-trainer"
    namespace: "ml"
    image: "ghcr.io/myorg/ml-trainer:latest"
    replicas: 1
    dependencies: {
        postgres: pg.PostgresConfig {
            provider: base.ProviderConfig {
                name: "neon"
                tier: "standard"
            }
            database: "mlflow"
        }
        redis: rd.RedisConfig {
            provider: base.ProviderConfig {
                name: "upstash"
            }
        }
        storage: {
            enabled: True
            provider: "s3"
            bucket_name: "ml-artifacts"
        }
    }
    resources: {
        requests: {cpu: "4", memory: "16Gi"}
        limits: {memory: "32Gi"}
    }
}

# Generate resources for one of the examples
# Uncomment the one you want to generate:

# items = app.generate_app_resources(simple_api)
# items = app.generate_app_resources(production_api)
items = app.generate_app_resources(staging_api)
# items = app.generate_app_resources(ml_worker)
