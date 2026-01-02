# Platform Operator Tiltfile
# Run: tilt up

# Build the operator image
docker_build(
    "yurikrupnik/platform-operator",
    ".",
    dockerfile="./Dockerfile",
    build_args={"BINARY": "platform-operator"},
    # Live update for faster iteration (rebuilds on source changes)
    live_update=[
        sync("./src", "/app/src"),
        sync("./Cargo.toml", "/app/Cargo.toml"),
        sync("./Cargo.lock", "/app/Cargo.lock"),
    ],
)

# Apply CRDs first (they must exist before the operator starts watching)
k8s_yaml("k8s-manifests/operator/crds.yaml")

# Apply operator manifests (namespace, RBAC, deployment)
k8s_yaml([
    "k8s-manifests/operator/deployment.yaml",
    "k8s-manifests/operator/rbac.yaml",
])

# Configure the operator resource with port forwarding and labels
k8s_resource(
    "platform-operator",
    labels=["operator"],
    resource_deps=["platformapps.platform.yurikrupnik.com"],
)

# Group CRDs under a label
k8s_resource(
    "platformapps.platform.yurikrupnik.com",
    labels=["crds"],
    new_name="crd-platformapps",
)
k8s_resource(
    "gitopsapps.platform.yurikrupnik.com",
    labels=["crds"],
    new_name="crd-gitopsapps",
)
k8s_resource(
    "crossplaneresources.platform.yurikrupnik.com",
    labels=["crds"],
    new_name="crd-crossplaneresources",
)
k8s_resource(
    "externalsecretconfigs.platform.yurikrupnik.com",
    labels=["crds"],
    new_name="crd-externalsecretconfigs",
)

# Optional: Create the namespace explicitly for the resource dependency
k8s_resource(
    "platform-system",
    labels=["infra"],
    new_name="namespace-platform-system",
)
