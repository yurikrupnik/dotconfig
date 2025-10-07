import .function_registry as fr
import .pipeline as pl

production_app_pipeline = lambda {
    _steps = [
        {
            functionName: "create_namespace"
            input: {
                name: "prod-app"
                labels: {
                    "app": "production-service"
                    "env": "production"
                    "managed-by": "kcl-pipeline"
                }
            }
        }
        {
            functionName: "create_secret"
            input: {
                name: "app-config"
                namespace: "prod-app"
                data: {
                    "database-host": "cHJvZC1kYi5leGFtcGxlLmNvbQ=="
                    "api-endpoint": "aHR0cHM6Ly9hcGkuZXhhbXBsZS5jb20="
                }
                labels: {
                    "app": "production-service"
                    "type": "config"
                }
            }
        }
        {
            functionName: "create_secret"
            input: {
                name: "app-credentials"
                namespace: "prod-app"
                data: {
                    "api-key": "c3VwZXItc2VjcmV0LWtleQ=="
                }
                labels: {
                    "app": "production-service"
                    "type": "credentials"
                }
            }
        }
    ]
    pl.execute_pipeline(_steps, None)
}

items = production_app_pipeline()
