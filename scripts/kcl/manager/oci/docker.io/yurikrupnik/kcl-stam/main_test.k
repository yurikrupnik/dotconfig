import .main as kind_cluster

test_default = lambda {
    # Default configuration should only include a control-plane node without ingress wiring.
    default_cluster = kind_cluster.Cluster {}
    assert default_cluster.name == "kind"
    assert len(default_cluster.nodes) == 1
    assert default_cluster.nodes[0].role == "control-plane"
    assert default_cluster.nodes[0].kubeadmConfigPatches is Undefined
    assert default_cluster.nodes[0].extraPortMappings is Undefined
}

test_ingress_cluster = lambda {
    # Enabling ingress with workers should attach patches and port mappings on the control-plane node.
    ingress_cluster = kind_cluster.Cluster {
        _name: "demo"
        _workers: 2
        _ingress: True
    }
    assert ingress_cluster.name == "demo"
    assert len(ingress_cluster.nodes) == 3
    #assert ingress_cluster.nodes[0].kubeadmConfigPatches is not Undefined
    assert len(ingress_cluster.nodes[0].kubeadmConfigPatches) == 1
    #assert ingress_cluster.nodes[0].extraPortMappings is not Undefined
    assert len(ingress_cluster.nodes[0].extraPortMappings) == 2
    assert ingress_cluster.nodes[0].extraPortMappings[0].hostPort == 80
    assert ingress_cluster.nodes[0].extraPortMappings[1].hostPort == 443
    assert ingress_cluster.nodes[0].extraPortMappings[0].$protocol == "TCP"
    assert ingress_cluster.nodes[1].role == "worker"
    assert ingress_cluster.nodes[2].role == "worker"
}

test_no_ingress_cluster = lambda {
    # Disabling ingress should skip port mappings even when workers are present.
    non_ingress_cluster = kind_cluster.Cluster {
        _workers: 2
        _ingress: False
    }
    assert len(non_ingress_cluster.nodes) == 3
    assert non_ingress_cluster.nodes[0].kubeadmConfigPatches is Undefined
    assert non_ingress_cluster.nodes[0].extraPortMappings is Undefined
}
