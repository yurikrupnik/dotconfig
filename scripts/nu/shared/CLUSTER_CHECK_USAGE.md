# Cluster Connectivity Check

Helper functions to validate Kubernetes context and cluster connectivity.

## Functions

### `check-cluster-connectivity`

Non-throwing function that returns a record with cluster status.

```nushell
use shared/shared.nu *

let result = check-cluster-connectivity

# Result structure:
# {
#   context: string    # Current kubectl context name
#   nodes: list        # List of node names
#   healthy: bool      # Whether cluster is reachable
#   error: string      # Error message if unhealthy
# }

if $result.healthy {
    print $"Connected to ($result.context) with ($result.nodes | length) nodes"
} else {
    print $"Error: ($result.error)"
}
```

### `require-cluster-connectivity`

Throws an error if cluster is not accessible. Use in scripts that require a working cluster.

```nushell
use shared/shared.nu *

# Will error if no cluster is available
let cluster = require-cluster-connectivity

# If we get here, cluster is healthy
print $"Working with nodes: ($cluster.nodes)"
```

## Usage Examples

### Guard clause at script start

```nushell
#!/usr/bin/env nu
use shared/shared.nu *

def deploy-app [] {
    require-cluster-connectivity  # Fails fast if cluster unavailable

    kubectl apply -f manifests/
}
```

### Conditional logic based on cluster state

```nushell
use shared/shared.nu *

let status = check-cluster-connectivity

if not $status.healthy {
    log warning $status.error
    log info "Attempting to start local cluster..."
    kind create cluster
}
```

### Check specific context

```nushell
use shared/shared.nu *

# Switch context first
kubectl config use-context kind-dev

let result = check-cluster-connectivity
if $result.context != "kind-dev" {
    error make { msg: "Expected kind-dev context" }
}
```

## Error Conditions

The function handles these error cases:

| Condition | `healthy` | `error` |
|-----------|-----------|---------|
| No context set | `false` | "No kubernetes context set..." |
| Cluster unreachable | `false` | "Failed to get nodes..." |
| No nodes in cluster | `false` | "No nodes found..." |
| Cluster accessible | `true` | `""` |
