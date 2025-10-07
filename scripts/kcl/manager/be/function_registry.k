import k8s.apimachinery.pkg.apis.meta.v1
import k8s.api.core.v1 as corev1
import external_secrets.v1 as es
import crossplane.apiextensions.v1 as crossplane

schema FunctionRegistry:
    functions: {str: any}

register_function = lambda registry: FunctionRegistry, name: str, fn: any -> FunctionRegistry {
    FunctionRegistry {
        functions: registry.functions | {(name) = fn}
    }
}

execute_function = lambda registry: FunctionRegistry, name: str, input: any -> any {
    assert name in registry.functions, "Function '${name}' not found in registry"
    registry.functions[name](input)
}

list_functions = lambda registry: FunctionRegistry -> [str] {
    [k for k, _ in registry.functions]
}

create_namespace = lambda input: any -> corev1.Namespace {
    corev1.Namespace {
        metadata: {
            name: input.name
            labels: input.labels if input?.labels else {}
        }
    }
}

create_secret = lambda input: any -> corev1.Secret {
    corev1.Secret {
        metadata: {
            name: input.name
            namespace: input.namespace
            labels: input.labels if input?.labels else {}
        }

        data: input.data if input?.data else {}
    }
}

create_cluster_secret_store = lambda input: any -> es.ClusterSecretStore {
    es.ClusterSecretStore {
        metadata: {
            name: input.name
        }
        spec: {
            conditions: [
                {
                    namespaces: input.namespaces if input?.namespaces else ["default"]
                }
            ]
            provider: {
                gcpsm: {
                    projectID: input.projectID
                    auth: {
                        secretRef: {
                            secretAccessKeySecretRef: {
                                key: input.secretKey
                                name: input.secretName
                            }
                        }
                    }
                }
            }
        }
    }
}

create_cluster_external_secret = lambda input: any -> es.ClusterExternalSecret {
    es.ClusterExternalSecret {
        metadata: {
            name: input.name
        }
        spec: {
            refreshTime: input.refreshTime if input?.refreshTime else '5m'
            externalSecretName: input.externalSecretName
            namespaceSelectors: [{}]
            externalSecretSpec: {
                refreshInterval: "1h"
                secretStoreRef: {
                    name: input.secretStoreName
                    kind: "ClusterSecretStore"
                }
                dataFrom: [
                    {
                        extract: {
                            key: secret
                            version: "latest"
                        }
                    } for secret in input.secrets
                ]
            }
        }
    }
}

create_xrd = lambda input: any -> crossplane.CompositeResourceDefinition {
    crossplane.CompositeResourceDefinition {
        metadata: {
            labels: {
                team: "platform"
                "app.kubernetes.io/part-of": "iac"
            }
            annotations: {
                "app.kubernetes.io/owner": input.owner
            }
            name: input.plural.lower() + "." + input.group
        }
        spec: {
            group: input.group
            claimNames.kind: input.claimKind
            claimNames.plural: input.claimPlural
            names: {
                kind: input.kind
                plural: input.plural
            }
            defaultCompositionRef.name: "default-" + input.kind.lower()
            versions: input.versions if input?.versions else [
                {
                    name: "v1alpha1"
                    served: True
                    referenceable: True
                    $schema.openAPIV3Schema: {
                        $type: "object"
                        properties: input.properties if input?.properties else {
                            spec: {
                                $type: "object"
                                properties: {}
                            }
                        }
                    }
                }
            ]
        }
    }
}

registry = FunctionRegistry {
    functions: {
        "create_namespace": create_namespace
        "create_secret": create_secret
        "create_cluster_secret_store": create_cluster_secret_store
        "create_cluster_external_secret": create_cluster_external_secret
        "create_xrd": create_xrd
    }
}
