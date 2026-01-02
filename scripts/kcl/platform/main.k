# Platform Stack Configuration
# Uses existing KCL modules from the registry

import k8s.api.core.v1 as corev1
import k8s.api.apps.v1 as appsv1
import gateway_api.api.v1 as gatewayv1
import argo_workflow.api.v1alpha1 as wfv1
import argo_rollouts.api.v1alpha1 as rolloutsv1
import dapr.api.v1alpha1 as daprv1
import istio.api.networking.v1 as istiov1
import tekton.api.v1 as tektonv1

# === Configuration Parameters ===
name: str = option("name") or "homelab"
namespace: str = option("namespace") or "apps"
enable_istio: bool = option("istio") or True
enable_dapr: bool = option("dapr") or True
enable_rollouts: bool = option("rollouts") or True

# === Platform Namespace ===
_namespaces = [
    corev1.Namespace {
        metadata.name = ns
        metadata.labels = {
            "istio.io/dataplane-mode" = "ambient" if enable_istio else "none"
        }
    }
    for ns in [namespace, "data", "workflows", "monitoring"]
]

# === Gateway API Configuration ===
_gateway = gatewayv1.Gateway {
    metadata = {
        name = "${name}-gateway"
        namespace = "istio-system"
    }
    spec = {
        gatewayClassName = "istio"
        listeners = [
            {
                name = "http"
                port = 80
                protocol = "HTTP"
                allowedRoutes.namespaces.from = "All"
            }
            {
                name = "https"
                port = 443
                protocol = "HTTPS"
                allowedRoutes.namespaces.from = "All"
                tls = {
                    mode = "Terminate"
                    certificateRefs = [{name = "tls-secret"}]
                }
            }
        ]
    }
}

# === Dapr Components ===
_dapr_pubsub = daprv1.Component {
    metadata = {
        name = "pubsub"
        namespace = "dapr-system"
    }
    spec = {
        type = "pubsub.nats"
        version = "v1"
        metadata = [
            {name = "natsURL", value = "nats://nats.data:4222"}
            {name = "jetstream", value = "true"}
        ]
    }
} if enable_dapr else None

_dapr_statestore = daprv1.Component {
    metadata = {
        name = "statestore"
        namespace = "dapr-system"
    }
    spec = {
        type = "state.redis"
        version = "v1"
        metadata = [
            {name = "redisHost", value = "redis.data:6379"}
            {name = "redisPassword", value = ""}
        ]
    }
} if enable_dapr else None

_dapr_bindings = daprv1.Component {
    metadata = {
        name = "data-lake"
        namespace = "dapr-system"
    }
    spec = {
        type = "bindings.aws.s3"
        version = "v1"
        metadata = [
            {name = "bucket", value = "data-lake"}
            {name = "endpoint", value = "http://minio.data:9000"}
            {name = "accessKey", value = "admin"}
            {name = "secretKey", value = "minio123"}
            {name = "region", value = "us-east-1"}
            {name = "forcePathStyle", value = "true"}
        ]
    }
} if enable_dapr else None

# === AI Workflow Template ===
_ai_workflow_template = wfv1.WorkflowTemplate {
    metadata = {
        name = "ai-job-template"
        namespace = "workflows"
    }
    spec = {
        entrypoint = "ai-pipeline"
        serviceAccountName = "argo-workflow"
        arguments.parameters = [
            {name = "model", default = "llama2"}
            {name = "input-data", default = "s3://data-lake/input"}
            {name = "output-data", default = "s3://data-lake/output"}
        ]
        templates = [
            {
                name = "ai-pipeline"
                dag.tasks = [
                    {
                        name = "fetch-data"
                        template = "s3-fetch"
                        arguments.parameters = [{name = "path", value = "{{workflow.parameters.input-data}}"}]
                    }
                    {
                        name = "process"
                        template = "inference"
                        dependencies = ["fetch-data"]
                        arguments.parameters = [{name = "model", value = "{{workflow.parameters.model}}"}]
                    }
                    {
                        name = "store-results"
                        template = "s3-store"
                        dependencies = ["process"]
                        arguments.parameters = [{name = "path", value = "{{workflow.parameters.output-data}}"}]
                    }
                ]
            }
            {
                name = "s3-fetch"
                inputs.parameters = [{name = "path"}]
                container = {
                    image = "amazon/aws-cli:latest"
                    command = ["aws"]
                    args = ["s3", "cp", "{{inputs.parameters.path}}", "/data", "--recursive", "--endpoint-url", "http://minio.data:9000"]
                    volumeMounts = [{name = "data", mountPath = "/data"}]
                    env = [
                        {name = "AWS_ACCESS_KEY_ID", value = "admin"}
                        {name = "AWS_SECRET_ACCESS_KEY", value = "minio123"}
                    ]
                }
            }
            {
                name = "inference"
                inputs.parameters = [{name = "model"}]
                container = {
                    image = "ollama/ollama:latest"
                    command = ["ollama"]
                    args = ["run", "{{inputs.parameters.model}}"]
                    resources.limits = {
                        memory = "8Gi"
                        cpu = "4"
                    }
                    volumeMounts = [{name = "data", mountPath = "/data"}]
                }
            }
            {
                name = "s3-store"
                inputs.parameters = [{name = "path"}]
                container = {
                    image = "amazon/aws-cli:latest"
                    command = ["aws"]
                    args = ["s3", "cp", "/data/output", "{{inputs.parameters.path}}", "--recursive", "--endpoint-url", "http://minio.data:9000"]
                    volumeMounts = [{name = "data", mountPath = "/data"}]
                    env = [
                        {name = "AWS_ACCESS_KEY_ID", value = "admin"}
                        {name = "AWS_SECRET_ACCESS_KEY", value = "minio123"}
                    ]
                }
            }
        ]
        volumeClaimTemplates = [
            {
                metadata.name = "data"
                spec = {
                    accessModes = ["ReadWriteOnce"]
                    resources.requests.storage = "10Gi"
                }
            }
        ]
        ttlStrategy = {
            secondsAfterCompletion = 3600
            secondsAfterSuccess = 600
            secondsAfterFailure = 86400
        }
    }
}

# === Argo Rollout with Istio ===
_app_rollout = rolloutsv1.Rollout {
    metadata = {
        name = "${name}-app"
        namespace = namespace
    }
    spec = {
        replicas = 3
        selector.matchLabels = {"app" = "${name}-app"}
        template = {
            metadata.labels = {"app" = "${name}-app"}
            metadata.annotations = {
                "dapr.io/enabled" = "true" if enable_dapr else "false"
                "dapr.io/app-id" = "${name}-app"
                "dapr.io/app-port" = "8080"
            }
            spec.containers = [{
                name = "app"
                image = "yurikrupnik/${name}:latest"
                ports = [{containerPort = 8080}]
                resources = {
                    requests = {memory = "128Mi", cpu = "100m"}
                    limits = {memory = "256Mi", cpu = "500m"}
                }
                env = [
                    {name = "DAPR_HTTP_PORT", value = "3500"}
                    {name = "DATA_LAKE_ENDPOINT", value = "http://minio.data:9000"}
                ]
            }]
        }
        strategy.canary = {
            steps = [
                {setWeight = 10}
                {pause = {duration = "5m"}}
                {setWeight = 30}
                {pause = {duration = "5m"}}
                {setWeight = 50}
                {pause = {duration = "5m"}}
                {setWeight = 100}
            ]
            trafficRouting.istio = {
                virtualServices = [{name = "${name}-vsvc", routes = ["primary"]}]
                destinationRule = {
                    name = "${name}-destrule"
                    stableSubsetName = "stable"
                    canarySubsetName = "canary"
                }
            }
            analysis = {
                templates = [{templateName = "success-rate"}]
                startingStep = 2
                args = [{name = "service-name", value = "${name}-app"}]
            }
        }
    }
} if enable_rollouts else None

# === Tekton Pipeline for CI ===
_ci_pipeline = tektonv1.Pipeline {
    metadata = {
        name = "${name}-ci"
        namespace = "tekton-pipelines"
    }
    spec = {
        params = [
            {name = "repo-url", type = "string"}
            {name = "revision", type = "string", default = "main"}
            {name = "image", type = "string"}
        ]
        workspaces = [
            {name = "source"}
            {name = "docker-credentials"}
        ]
        tasks = [
            {
                name = "clone"
                taskRef.name = "git-clone"
                workspaces = [{name = "output", workspace = "source"}]
                params = [
                    {name = "url", value = "$(params.repo-url)"}
                    {name = "revision", value = "$(params.revision)"}
                ]
            }
            {
                name = "build-push"
                taskRef.name = "buildah"
                runAfter = ["clone"]
                workspaces = [
                    {name = "source", workspace = "source"}
                    {name = "dockerconfig", workspace = "docker-credentials"}
                ]
                params = [
                    {name = "IMAGE", value = "$(params.image)"}
                ]
            }
            {
                name = "deploy"
                taskRef.name = "kubernetes-actions"
                runAfter = ["build-push"]
                params = [
                    {name = "script", value = "kubectl argo rollouts set image ${name}-app app=$(params.image)"}
                ]
            }
        ]
    }
}

# === HTTPRoute for Gateway API ===
_http_route = gatewayv1.HTTPRoute {
    metadata = {
        name = "${name}-route"
        namespace = namespace
    }
    spec = {
        parentRefs = [{
            name = "${name}-gateway"
            namespace = "istio-system"
        }]
        hostnames = ["${name}.local"]
        rules = [{
            matches = [{path = {type = "PathPrefix", value = "/"}}]
            backendRefs = [{
                name = "${name}-app"
                port = 8080
            }]
        }]
    }
}

# === Output all resources ===
items = [
    r for r in [
        *_namespaces
        _gateway
        _dapr_pubsub
        _dapr_statestore
        _dapr_bindings
        _ai_workflow_template
        _app_rollout
        _ci_pipeline
        _http_route
    ] if r != None
]
