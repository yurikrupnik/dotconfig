import k8s.api.apps.v1 as apps
import k8s.api.core.v1 as s
import k8s.api.admissionregistration.v1 as admissionregistration
import manifests
import .config as config

schema IstioVirtualService:
    apiVersion: str = "networking.istio.io/v1beta1"
    kind: str = "VirtualService"
    metadata: {str:any}
    spec: {str:any}

schema IstioGateway:
    apiVersion: str = "networking.istio.io/v1beta1"
    kind: str = "Gateway"
    metadata: {str:any}
    spec: {str:any}

_validatePolicy = admissionregistration.ValidatingAdmissionPolicy {
    metadata.name: "my-app"
    metadata.labels = {
        app = "my-app"
    }
    spec.failurePolicy = "Fail"
    spec.matchConstraints.resourceRules = [
        {
            apiGroups = ["yurikrupnik.com"]
            apiVersions = ["*"]
            resources = ["appclaims"]
            operations = ["CREATE", "UPDATE"]
        }
    ]
    spec.validations = [
        {
            message = "`spec.parameters.scaling.enabled` must be set to `true`"
            expression = """\
has(object.spec.parameters.scaling) &&
has(object.spec.parameters.scaling.enabled) &&
object.spec.parameters.scaling.enabled
"""
        }
        {
            message = "`spec.parameters.scaling.min` must be greater than `1`"
            expression = """\
has(object.spec.parameters.scaling) &&
object.spec.parameters.scaling.min > 1
"""
        }
    ]
}

_valid = admissionregistration.ValidatingAdmissionPolicyBinding {
    metadata.name = "my-app"
    metadata.labels = {
        app: "my-app"
    }
    spec.policyName = "aris"
    spec.validationActions = ["Deny"]
    spec.matchResources.namespaceSelector.matchLabels = {
        app: "my-app"
    }
}

_secret = s.Secret {
    metadata.name: "my-app"
    metadata.labels = {
        app = "my-app"
    }
}

# _serviceAccount = s.ServiceAccount {
#     metadata.name = "my-app"
#     metadata.labels = {
#         app = "my-app"
#     }
# }
_deployment = apps.Deployment {
    metadata.name = "my-app"
    metadata.labels = {
        app = "${metadata.name}"
        team = "my-team"
    }
    spec = {
        replicas = 3
        selector.matchLabels = {
            app = "${metadata.name}"
        }
        template.metadata.labels = {
            app = "${metadata.name}"
        }
        template.spec.containers = [
            {
                name = "my-app"
                image = "my-app:latest"
                ports = [
                    {
                        containerPort = 8080
                    }
                ]
            }
        ]
    }
}

_service = s.Service {
    metadata.name: "my-app"
    #metadata.name = "${metadata.name}"
    metadata.labels: {
     #   app: "my-app"
        app = "${metadata.name}"
    }
}

_create_istio_ingress = lambda name: str, host: str, gateway: str -> [any] {
    _gateway = gateway if gateway else "istio-system/gateway"
    [
        IstioVirtualService {
            apiVersion = "networking.istio.io/v1beta1"
            kind = "VirtualService"
            metadata = {
                name = name
                labels = {
                    app = name
                    #app = "${metadata.name}"
                }
            }
            spec = {
                hosts = [host]
                gateways = [_gateway]
                http = [
                    {
                        match = [
                            {
                                uri = {
                                    prefix = "/"
                                }
                            }
                        ]
                        route = [
                            {
                                destination = {
                                    host = name
                                    port = {
                                        number = 80
                                    }
                                }
                            }
                        ]
                    }
                ]
            }
        }
    ]
}

_create_app_resources = lambda name: str -> [any] {
    [
        apps.Deployment {
            metadata.name = $name
            metadata.labels = {
                app = "${metadata.name}"
                team = "my-team"
                version = "v1"
            }
            metadata.annotations = {
                "kubernetes.io/change-cause" = "Deployed via KCL"
            }
            spec = {
                replicas = 3
                revisionHistoryLimit = 10
                strategy = {
                    $type = "RollingUpdate"
                    rollingUpdate = {
                        maxSurge = 1
                        maxUnavailable = 0
                    }
                }
                selector.matchLabels = {
                    app = "${metadata.name}"
                }
                template.metadata.labels = {
                    app = "${metadata.name}"
                    version = "v1"
                }
                template.spec = {
                    securityContext = {
                        runAsNonRoot = True
                        runAsUser = 1000
                        fsGroup = 1000
                    }
                    containers = [
                        {
                            name = $name
                            image = "${name}:latest"
                            imagePullPolicy = "IfNotPresent"
                            ports = [
                                {
                                    name = "http"
                                    containerPort = 8080
                                    protocol = "TCP"
                                }
                            ]
                            env = [
                                {
                                    name = "PORT"
                                    value = "8080"
                                }
                                {
                                    name = "APP_NAME"
                                    value = $name
                                }
                            ]
                            resources = {
                                requests = {
                                    cpu = "100m"
                                    memory = "128Mi"
                                }
                                limits = {
                                    memory = "512Mi"
                                }
                            }
                            livenessProbe = {
                                httpGet = {
                                    path = "/healthz"
                                    port = 8080
                                    scheme = "HTTP"
                                }
                                initialDelaySeconds = 30
                                periodSeconds = 10
                                timeoutSeconds = 5
                                failureThreshold = 3
                            }
                            readinessProbe = {
                                httpGet = {
                                    path = "/ready"
                                    port = 8080
                                    scheme = "HTTP"
                                }
                                initialDelaySeconds = 10
                                periodSeconds = 5
                                timeoutSeconds = 3
                                failureThreshold = 3
                            }
                            securityContext = {
                                allowPrivilegeEscalation = False
                                readOnlyRootFilesystem = True
                                runAsNonRoot = True
                                runAsUser = 1000
                                capabilities = {
                                    drop = ["ALL"]
                                }
                            }
                        }
                    ]
                }
            }
        }
        s.Service {
            metadata.name = $name
            metadata.labels = {
                app = $name
            }
            spec = {
                $type = "ClusterIP"
                selector = {
                    app = $name
                }
                ports = [
                    {
                        name = "http"
                        port = 80
                        targetPort = "http"
                        protocol = "TCP"
                    }
                ]
            }
        }
    ]

}

schema App:
    _app: str = "nginx"
    _labels: {str:str} = {
        # app = "${metadata.name}"
        team = "my-team"
    }
    _deplyment: _deployment
    _service: _service

create_app_with_service = lambda name: str, withIstio: bool, host: str, gateway: str -> [any] {
    _baseResources = [
        apps.Deployment {
            metadata.name = name
            metadata.labels = {
                app = name
                team = "platform"
                "managed-by" = "kcl"
            }
            spec = {
                replicas = 3
                selector.matchLabels = {
                    app = name
                }
                template.metadata.labels = {
                    app = name
                }
                template.spec.containers = [
                    {
                        name = name
                        image = "${name}:latest"
                        ports = [
                            {
                                containerPort = 8080
                            }
                        ]
                    }
                ]
            }
        }
        s.Service {
            metadata.name = name
            metadata.labels = {
                app = name
            }
            spec = {
                selector = {
                    app = name
                }
                ports = [
                    {
                        port = 80
                        targetPort = 8080
                    }
                ]
            }
        }
    ]

    _istioResources = _create_istio_ingress(name, host, gateway) if withIstio else []

    _baseResources + _istioResources
}

_withIstio = option("istio", default=False)
_host = option("host", default="${config.config.kind.lower()}.example.com")
_gateway = option("gateway", default="istio-system/gateway")

_items = create_app_with_service(config.config.kind.lower(), _withIstio, _host, _gateway)

items = _items
