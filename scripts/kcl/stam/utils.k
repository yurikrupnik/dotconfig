import .cluster

create_worker_nodes = lambda workers: int -> [any] {
    [cluster.Node {
        role: "worker"
    } for i in range(workers)]
}

create_control_plane = lambda name: str, ingress: bool -> cluster.Node {
    cluster.Node {
        role: "control-plane"
        if ingress:
            kubeadmConfigPatches: ['''
kind: InitConfiguration
nodeRegistration:
  kubeletExtraArgs:
    node-labels: "ingress-ready=true"
''']
            extraPortMappings: [
                cluster.PortMapping {
                    containerPort: 80
                    hostPort: 80
                    $protocol: "TCP"
                }
                cluster.PortMapping {
                    containerPort: 443
                    hostPort: 443
                    $protocol: "TCP"
                }
            ]
    }
}