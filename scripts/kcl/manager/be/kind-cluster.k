schema PortMapping:
    containerPort: int
    hostPort: int
    $protocol?: "TCP" | "UDP" = "TCP"

schema Node:
    role: "control-plane" | "worker"
    # kubeadmConfigPatches?: [KubeadmInitPatch]
    kubeadmConfigPatches?: [str]
    extraPortMappings?: [PortMapping]

_ingress_init_patch = '''
kind: InitConfiguration
nodeRegistration:
  kubeletExtraArgs:
    node-labels: "ingress-ready=true"
'''

schema Cluster:
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
                extraPortMappings: [
                    PortMapping {
                        containerPort: 80
                        hostPort: 80
                        $protocol: "TCP"
                    }
                    PortMapping {
                        containerPort: 443
                        hostPort: 443
                        $protocol: "TCP"
                    }
                ]
        }
    ] + [Node {
        role: "worker"
    } for i in range(_workers)]

items = [Cluster {}]

