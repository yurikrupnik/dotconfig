import .function_registry as fr
import .pipeline as pl
import .crossplane_function as cf

example_simple_function_execution = lambda {
    _namespace = fr.execute_function(fr.registry, "create_namespace", {
        name: "example-ns"
        labels: {"env": "development", "team": "platform"}
    })

    [_namespace]
}

example_pipeline_execution = lambda {
    _steps = [
        {
            functionName: "create_namespace"
            input: {
                name: "pipeline-example-ns"
                labels: {"env": "production"}
            }
        }
        {
            functionName: "create_secret"
            input: {
                name: "pipeline-example-secret"
                namespace: "pipeline-example-ns"
                data: {"api-key": "secret-value"}
                labels: {"app": "my-app"}
            }
        }
        {
            functionName: "create_cluster_secret_store"
            input: {
                name: "pipeline-example-store"
                projectID: "my-gcp-project"
                secretName: "gcp-credentials"
                secretKey: "creds"
                namespaces: ["pipeline-example-ns", "default"]
            }
        }
        {
            functionName: "create_cluster_external_secret"
            input: {
                name: "pipeline-example-external-secret"
                secretStoreName: "pipeline-example-store"
                externalSecretName: "app-secrets"
                refreshTime: "10m"
                secrets: ["database-password", "api-keys", "certificates"]
            }
        }
    ]

    pl.execute_pipeline(_steps, None)
}

example_crossplane_composition = lambda {
    _composition = cf.create_dynamic_composition("create_xrd", {
        kind: "Application"
        plural: "applications"
        group: "platform.example.com"
        claimKind: "ApplicationClaim"
        claimPlural: "applicationclaims"
        owner: "platform-team@example.com"
        properties: {
            spec: {
                $type: "object"
                properties: {
                    region: {
                        $type: "string"
                        description: "Deployment region"
                    }
                    replicas: {
                        $type: "integer"
                        description: "Number of replicas"
                        minimum: 1
                    }
                }
            }
        }
    })

    [_composition]
}

example_custom_function_registration = lambda {
    _custom_fn = lambda input: any -> {str:any} {
        {
            apiVersion: "v1"
            kind: "CustomResource"
            metadata: {
                name: input.name
                labels: {
                    environment: input.environment
                    "managed-by": "kcl-function-registry"
                }
            }
            spec: {
                environment: input.environment
            }
        }
    }

    _extended_registry = fr.register_function(fr.registry, "create_custom_resource", _custom_fn)

    _result = fr.execute_function(_extended_registry, "create_custom_resource", {
        name: "my-custom-resource"
        environment: "staging"
    })

    [_result]
}

example_full_crossplane_function = lambda {
    _steps = [
        {
            functionName: "create_namespace"
            input: {
                name: "app-namespace"
                labels: {"team": "platform", "managed-by": "crossplane"}
            }
        }
        {
            functionName: "create_cluster_secret_store"
            input: {
                name: "app-secret-store"
                projectID: "production-project"
                secretName: "secret-manager-credentials"
                secretKey: "service-account-key"
                namespaces: ["app-namespace"]
            }
        }
        {
            functionName: "create_cluster_external_secret"
            input: {
                name: "app-external-secrets"
                secretStoreName: "app-secret-store"
                externalSecretName: "application-secrets"
                refreshTime: "5m"
                secrets: ["database-url", "redis-password", "jwt-secret"]
            }
        }
        {
            functionName: "create_xrd"
            input: {
                kind: "DatabaseInstance"
                plural: "databaseinstances"
                group: "database.example.com"
                claimKind: "DatabaseInstanceClaim"
                claimPlural: "databaseinstanceclaims"
                owner: "database-team@example.com"
            }
        }
    ]

    _oxr = {
        spec: {
            parameters: {
                projectID: "production-project"
                region: "us-central1"
                environment: "production"
            }
        }
    }

    cf.create_crossplane_function(_steps, _oxr)
}

items = example_pipeline_execution()
