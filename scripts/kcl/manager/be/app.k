import k8s.api.apps.v1 as apps
import k8s.api.core.v1 as s
import k8s.api.admissionregistration.v1 as admissionregistration
import manifests

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
    metadata.labels: {
        app: "my-app"
    }
}

schema App:
    _app: str = "nginx"
    _labels: {str:str} = {
        # app = "${metadata.name}"
        team = "my-team"
    }
    _deplyment: apps.Deployment {
        metadata: {
            name: _app
        }
    }
    _service: s.Service {
        metadata: {
            name: _app
            labels: {
                app: "my-app"
            }
        }
    }

# service_account: s.ServiceAccount =
_items = [
    # App {
    #     deplyment:
    # }
    # _serviceAccount
    _deployment
    _service
]
# _secret, _service, _serviceAccount, _validatePolicy]
manifests.yaml_stream(_items)
