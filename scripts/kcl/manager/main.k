import stam
import external_secrets.v1 as es
# import external_secrets.v1alpha1 as es
import crossplane.apiextensions.v1 as crossplane_apiextensions_v1
import crossplane.pkg.v1 as ex_fn
import be.config as config
import k8s.apimachinery.pkg.apis.meta.v1
import k8s.api.core.v1 as corev1

NS_LABEL_KEY = "extsecrets.yurikrupnik.io/enabled"
NS_LABEL_VAL = "true"

_metadata = lambda name: str -> any {
    {
        annotations = {"krm.kcl.dev/composition-resource-name" = name}
    }
}
#oxr = option("params").oxr # observed composite resource
#_ocds = option("params").ocds # observed composed resources
#_dxr = option("params").dxr # desired composite resource
#dcds = option("params").dcds # desired composed resources
#_ctx = option("params").ctx
#_path= option("PATH")
#_env= option("env")
# _crossplane_provider = crossplane_apiextensions_v1.Provider {}
_composite = crossplane_apiextensions_v1.CompositeResourceDefinition {
    metadata = {
        labels = {
            team = "platform"
            "app.kubernetes.io/part-of" = "iac"
        }
        annotations = {
            "app.kubernetes.io/owner": config.config.owner
        }
        name = config.config.plural.lower() + "." + config.config.group
    }
    spec = {
        group = config.config.group
        claimNames.kind = config.config.claimNamesKind
        claimNames.plural = config.config.claimNamesPlural
        names = {
            kind = config.config.kind
            plural = config.config.plural
        }
        defaultCompositionRef.name = "gcp-" + config.config.kind.lower()
        versions = [
            {
                name = "v1alpha1"
                served = True
                referenceable = True
                # does not work
                # TODO add back with registry params
                additionalPrinterColumns = [
                    {
                        name = "url"
                        $type = "string"
                        jsonPath = ".status.url"
                    }
                ]
                $schema.openAPIV3Schema = {
                    $type = "object"
                    properties = {
                        spec = {
                            $type = "object"
                            required = []
                            properties = {
                                location = {
                                    $type = "string"
                                    description = "Location where the registry should be created."
                                    examples: ["europe-west1 (GCP)", "eu-west-1 (AWS)"]
                                }
                            }
                        }
                        # TODO add back with registry params
                        status = {
                            $type = "object"
                            properties = {
                                url.$type = "string"
                            }
                        }
                    }
                }
            }
        ]
    }
}
# schema CpFunctionProps:
# # _metadata
#     package: str
# cp_function = lambda props: ex_fn.PkgCrossplaneIoV1FunctionSpec -> ex_fn.Function {
#     ex_fn.Function {
#         metadata.name: props.name
#         spec: {
#             package: props.package
#         }
#     }
# }
# _function = cp_function({
#     name: "upbound-provider-gcp-storage"
#     package: "xpkg.upbound.io/upbound/provider-gcp-storage:v2"
# })
# # _function = ex_fn.Function {
# #     metadata.name: "upbound-provider-gcp-storage"
# #     spec: {
# #         package: "xpkg.upbound.io/upbound/provider-gcp-storage:v2"
# #     }
# # }
# _provider = ex_fn.Provider {
#     metadata.name: "crossplane-contrib-function-kcl"
#     spec: {
#         package: "xpkg.upbound.io/crossplane-contrib/function-kcl:v0.11.5"
#     }
# }
# import k8s.api
# x: es.SecretStore,
schema ExternalSecretProjectProps:
    _metadata?: v1.ObjectMeta
    # _external_secret: es.
    _external_secret: es.ExternalSecretsIoV1ExternalSecretSpec = {
        refreshInterval: "5m"
    }
    data: any = {
        dsa: es.ExternalSecretsIoV1SecretStoreSpecProvider
    }

nx_sd = corev1.Namespace {
    metadata: {
        name: "default"
        labels: {
            (NS_LABEL_KEY): NS_LABEL_VAL
        }
    }
}
# _sad: }
generate_external_secret_connection = lambda y: int, dsa: ExternalSecretProjectProps -> [any] {
    ss = es.SecretStore {
        metadata: {
            name: "manager-secrets"
            namespace: "default"
        }
        spec: {
            # conditions: [
            #     {namespaces: [
            #         "apps"
            #         "crossplane-system"
            #     ]}
            # ]
            provider: {
                gcpsm: {
                    projectID: "playground-447016"
                    auth: {
                        secretRef: {
                            secretAccessKeySecretRef: {
                                name: "secret-puller"
                                key: "creds"
                            }
                        }
                    }
                }
            }
        }
    }
    sd = es.ClusterSecretStore {
        metadata: {
            name: "lol-secrets"
        }
        spec: {
            conditions: [
                {
                    namespaces: ["default", "apps"]
                }
            ]
            provider: {
                gcpsm: {
                    projectID: "playground-447016"
                    auth: {
                        secretRef: {
                            secretAccessKeySecretRef: {
                                key: "creds"
                                name: "secret-puller"
                            }
                        }
                    }
                }
            }
        }
    }
    es_item = es.ClusterExternalSecret {
        metadata: {
            name: "lol-secrets"
        }
        spec: {
            refreshTime: '5m'
            externalSecretName: "ass-dd-shit"
            # namespaceSelectors:
            # - matchLabels:
            #     cool: label
            namespaceSelectors: [{}]
            # matchExpressions: [{key: "app"}]
            # matchLabels: {
            #     cool: "label"
            # }
            externalSecretSpec: {
                refreshInterval: "1h"
                secretStoreRef: {
                    name: "lol-secrets"
                    kind: "ClusterSecretStore"
                }
                # data: [
                #     {
                #         secretKey: "ass-secret"
                #         remoteRef: {
                #             key: "github-secret"
                #         }
                #     }
                # ]
                # target: {
                #     name: "aris-god"
                #     creationPolicy: "Owner"
                # }
                # template: {type: "kubernetes.io/dockerconfigjson"}
                dataFrom: [
                    {
                        extract: {
                            key: "manager-secrets"
                            version: "latest"
                        }
                    }
                    {
                        extract: {
                            key: "app-secrets"
                            version: "latest"
                        }
                    }
                ]
            }
        }
    }
    # externalSecretSpec: {
    #     secretStoreRef: {
    #         name: "secret-store"
    #         kind: "ClusterSecretStore"
    #     }
    #     # target: {
    #     #     name: "mongo-uri"
    #     #     creationPolicy: "Owner"
    #     # }
    #     # template: {type: "kubernetes.io/dockerconfigjson"}
    #     # data: [
    #     #     {
    #     #         secretKey: ".dockerconfigjson"
    #     #         remoteRef: {version: "2", key: "container-puller"}
    #     #     }
    #     # ]
    #     dataFrom: [
    #         {
    #             extract: {
    #                 key: "manager-secrets"
    #                 version: "latest"
    #             }
    #         }
    #         {
    #             extract: {
    #                 key: "app-secrets"
    #                 version: "latest"
    #             }
    #         }
    #     ]
    # - key: provider-key
    # version: provider-key-version
    # property: provider-key-property
    # refreshTime:
    [
        sd
        es_item
    ]
}
# _provider
# _function
# es.ExternalSecret {
#     metadata: ss.metadata
#     spec: {
#         refreshInterval: dsa._external_secret.refreshInterval
#         secretStoreRef: {
#             kind: "SecretStore"
#             name: "manager-secrets"
#         }
#         target: {
#             # name of the k8s Secret to be created
#             name: "dam-secrets"
#             # Enum with values: 'Owner', 'Merge', or 'None'
#             creationPolicy: "Owner"
#         }
#         dataFrom: [
#             {
#                 extract: {
#                     key: "manager-secrets"
#                     version: "latest"
#                 }
#             }
#             {
#                 extract: {
#                     key: "app-secrets"
#                     version: "latest"
#                 }
#             }
#         ]
#     }
# }
# ]
# # externalSecretName: "hello-world-es"
#                refreshInterval: "5m"
# # refreshTime: "1h"
# externalSecretSpec: {
#     secretStoreRef: {
#         name: "secret-store"
#         kind: "ClusterSecretStore"
#     }
#     # target: {
#     #     name: "mongo-uri"
#     #     creationPolicy: "Owner"
#     # }
#     # template: {type: "kubernetes.io/dockerconfigjson"}
#     # data: [
#     #     {
#     #         secretKey: ".dockerconfigjson"
#     #         remoteRef: {version: "2", key: "container-puller"}
#     #     }
#     # ]
#     dataFrom: [
#         {
#             extract: {
#                 key: "manager-secrets"
#                 version: "latest"
#             }
#         }
#         {
#             extract: {
#                 key: "app-secrets"
#                 version: "latest"
#             }
#         }
#     ]
# refreshInterval: dsa._external_secret.refreshInterval
# secretStoreRef: {
#     kind: "SecretStore"
#     name: "secret-store"
# }
# target: {
# name of the k8s Secret to be created
#     name: "config-secrets"
#     # Enum with values: 'Owner', 'Merge', or 'None'
#     creationPolicy: "Owner"
# }
# dataFrom: [
#     {
#         extract: {
#             key: "manager-secrets"
#             version: "latest"
#         }
#     }
#     {
#         extract: {
#             key: "app-secrets"
#             version: "latest"
#         }
#     }
# ]
# namespace: "default"
#onepasswordSDK: {
#    # uique to project
#    vault: "vp-yurikrupnik"
#    auth: {
#        serviceAccountSecretRef: {
#            name: "1pass-puller"
#            key: "creds"
#            namespace: "default"
#        }
#    }
#}
# Example to retrieve variables from "xr"; update as needed
# _region = "us-east-1"
# if oxr.spec?.parameters?.region:
#     _region = oxr.spec.parameters.region
_asd = ExternalSecretProjectProps {
    _external_secret: {
        #refreshInterval: "22m"
        secretStoreRef: {
            kind: "SecretStore"
            name: "secret-store"
        }
        target: {
            # name of the k8s Secret to be created
            name: "omg"
        }
        # Enum with values: 'Owner', 'Merge', or 'None'
        # creationPolicy: "Owner"
        dataFrom: [
            {
                extract: {
                    key: "manager-secrets"
                    version: "latest"
                }
            }
            {
                extract: {
                    key: "app-secrets"
                    version: "latest"
                }
            }
            {
                extract: {
                    key: "github-secret"
                    version: "latest"
                }
            }
        ]
    }
    _metadata: {
        name: "nm1"
        namespace: "default"
    }
}
_items = generate_external_secret_connection(1, _asd)
# Example S3 Bucket managed resource configuration; update as needed
# s3v1beta2.Bucket{
#     metadata: _metadata("my-bucket")
#     spec: {
#         forProvider: {
#             region: _region
#         }
#     }
# }
items = _items
#items = [_dss, _ds, generate_external_secret_connection(1)]
