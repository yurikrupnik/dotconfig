# Platform Dependencies Package Manager
# Main entry point - generates all resources from app manifest
#
# Usage:
#   kcl run scripts/kcl/platform/deps/main.k -D name=my-app -D namespace=prod -D image=my-app:v1
#
# Or with full manifest:
#   kcl run scripts/kcl/platform/deps/main.k -D manifest='{"name":"my-app","image":"img:v1","dependencies":{"postgres":{"provider":{"name":"neon"}}}}'

import .base as base
import .postgres as pg
import .redis as rd
import .app

# CLI options
_name = option("name", default="my-app")
_namespace = option("namespace", default="default")
_image = option("image", default="nginx:latest")
_replicas = option("replicas", default=1)
_port = option("port", default=8080)

# Dependency options (can be JSON strings)
_postgres = option("postgres")
_redis = option("redis")
_provider = option("provider", default="internal")  # internal, neon, supabase, upstash, etc.

# Full manifest option (overrides individual options)
_manifest_json = option("manifest")

# Build manifest from options
_build_manifest = lambda -> app.AppManifest {
    _pg_config = None
    _rd_config = None

    # Parse postgres config
    if _postgres:
        _pg_config = pg.PostgresConfig {
            provider: base.ProviderConfig {
                name: _provider
                internal: _provider == "internal" or _provider == "cnpg"
            }
        }

    # Parse redis config
    if _redis:
        _rd_config = rd.RedisConfig {
            provider: base.ProviderConfig {
                name: _provider if _provider in ["upstash", "redis-cloud"] else "internal"
                internal: _provider == "internal"
            }
        }

    app.AppManifest {
        name: _name
        namespace: _namespace
        image: _image
        replicas: int(_replicas)
        port: int(_port)
        dependencies: {
            postgres: _pg_config
            redis: _rd_config
        }
    }
}

# Generate resources
_manifest = _build_manifest()
items = app.generate_app_resources(_manifest)
