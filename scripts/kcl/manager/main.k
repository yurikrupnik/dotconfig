# schema PortMapping:
#     containerPort: int
#     hostPort: int
#     $protocol?: "TCP" | "UDP" = "TCP"
# schema KubeadmInitPatch:
#     kind: "InitConfiguration"
# # nodeRegistration: {
# #     kubeletExtraArgs?: {"node-labels": str}
# # }
import manifests

schema Node:
    role: "control-plane" | "worker"
    # kubeadmConfigPatches?: [KubeadmInitPatch]
    kubeadmConfigPatches?: [str]
    extraPortMappings?: [PortMapping]

# # metadata = {
# #     labels = {
# #         db = "postgresql"
# #         provider = _provider
# #     }
# #     name = _provider + "-postgresql"
# # }
# data = {
#     kind: "Cluster"
#     apiVersion: "kind.x-k8s.io/v1alpha4"
#     name: "aris"
#     nodes: [
#         {
#             role: "control-plane"
#             kubeadmConfigPatches: [{
#                 kind: "InitConfiguration"
#                 nodeRegistration.kubeletExtraArgs = {"node-labels": "ingress-ready=true"}
#             }]
#             extraPortMappings: [
#                 {
#                     containerPort: 80
#                     hostPort: 80
#                     $protocol: "TCP"
#                 }
#                 {
#                     containerPort: 443
#                     hostPort: 443
#                     $protocol: "TCP"
#                 }
#             ]
#         }
#         {
#             role: "worker"
#         }
#         {
#             role: "worker"
#         }
#     ]
# }
# schema composition:
#     _provider: str
#     _source: str
#     _statusConditionHooks: any
#     _serverApiVersion: str
#     _serverKind: str
#     apiVersion = "apiextensions.crossplane.io/v1"
#     kind = "Composition"
#     metadata = {
#         labels = {
#             db = "postgresql"
#             provider = _provider
#         }
#         name = _provider + "-postgresql"
#     }
_ingress_init_patch = '''
kind: InitConfiguration
nodeRegistration:
  kubeletExtraArgs:
    node-labels: "ingress-ready=true"
'''
#_ingress: bool = option("ingress", default=True)
schema PortMapping:
    containerPort: int
    hostPort: int
    $protocol?: "TCP" | "UDP" = "TCP"

schema Cluster:
    #_provider?: str
    _name: str = option("name", default="aris")
    _ingress: bool = option("ingress", default=True)
    _slaves: int = option("slaves", default=0)
    _workers: int = option("workers", default=0)
    kind?: "Cluster" = "Cluster"
    #labels?: {any:any} = {dam: "aris"}
    apiVersion?: "kind.x-k8s.io/v1alpha4" = "kind.x-k8s.io/v1alpha4"
    name: str = _name
    nodes: [Node] = [
        Node {
            role: "control-plane"
            if _ingress and _workers > 0:
                kubeadmConfigPatches: [_ingress_init_patch]
                # NOTE: the field name is `protocol` (no `$` when setting a value)
                extraPortMappings: [
                    PortMapping {
                        containerPort: 80
                        hostPort: 80
                        protocol: "TCP"
                    }
                    PortMapping {
                        containerPort: 443
                        hostPort: 443
                        protocol: "TCP"
                    }
                ]
        }
    ] + [Node {
        role: "worker"
    } for i in range(_workers)]

# - metadata.labels = { app = "my-app" } uses assignment
#   - metadata.labels: { app: "my-app" } uses merge operator
# # if _ha -
#_con = Cluster {
#   _ingress: option("ingress", default=False)
#  _workers: option("workers", default=0)
# [Node {
#     role: "worker"
# } for _ in range(option("workers", default=2))]
# _name: option("name", default="aris")
#}
# #
# _con.name = "nir"
# _con.nodes[0].role = "worker"
# _con.labels = {app: "my-appa"}
items = [Cluster {}]
# yaml.parse()
# # name: _name
# # nodes: [Node {role: "control-plane"}] + _workers
# dam: {any:any} = {
#     "Ard": "as"
# }
# manifests.yaml_stream(_items)
