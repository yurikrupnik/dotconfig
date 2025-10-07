# KCL Dynamic Function System

A dynamic function registry and pipeline system for KCL, designed to work seamlessly with Crossplane compositions.

## Features

- **Function Registry**: Register and execute functions dynamically from a centralized registry
- **Pipeline Execution**: Chain multiple functions together in a pipeline
- **Crossplane Integration**: Use as Crossplane KCL functions in compositions
- **Built-in Functions**: Pre-configured functions for common Kubernetes resources

## Core Components

### 1. Function Registry (`function_registry.k`)

The registry provides a centralized way to manage and execute functions.

**Built-in Functions:**
- `create_namespace` - Create Kubernetes namespaces
- `create_secret` - Create Kubernetes secrets
- `create_cluster_secret_store` - Create External Secrets ClusterSecretStore
- `create_cluster_external_secret` - Create External Secrets ClusterExternalSecret
- `create_xrd` - Create Crossplane CompositeResourceDefinitions

**Usage:**
```kcl
import .function_registry as fr

result = fr.execute_function(fr.registry, "create_namespace", {
    name: "my-namespace"
    labels: {"env": "production"}
})

extended_registry = fr.register_function(fr.registry, "my_function", my_custom_fn)
```

### 2. Pipeline (`pipeline.k`)

Execute multiple functions in sequence.

**Usage:**
```kcl
import .pipeline as pl

steps = [
    {
        functionName: "create_namespace"
        input: {name: "app-ns"}
    }
    {
        functionName: "create_secret"
        input: {name: "app-secret", namespace: "app-ns"}
    }
]

results = pl.execute_pipeline(steps, None)
```

### 3. Crossplane Function (`crossplane_function.k`)

Use the function system with Crossplane compositions.

**Usage:**
```kcl
import .crossplane_function as cf

steps = [
    {functionName: "create_namespace", input: {name: "app"}}
    {functionName: "create_xrd", input: {...}}
]

oxr = option("params").oxr
output = cf.create_crossplane_function(steps, oxr)
```

**Create Dynamic Composition:**
```kcl
composition = cf.create_dynamic_composition("create_secret", {
    name: "my-secret"
    namespace: "default"
})
```

## Examples

### Simple Function Execution

```kcl
import .function_registry as fr

namespace = fr.execute_function(fr.registry, "create_namespace", {
    name: "production"
    labels: {"environment": "prod"}
})
```

### Pipeline with Multiple Resources

```kcl
import .pipeline as pl

pipeline_steps = [
    {
        functionName: "create_cluster_secret_store"
        input: {
            name: "secret-store"
            projectID: "my-gcp-project"
            secretName: "gcp-creds"
            secretKey: "key"
            namespaces: ["default", "apps"]
        }
    }
    {
        functionName: "create_cluster_external_secret"
        input: {
            name: "app-secrets"
            secretStoreName: "secret-store"
            externalSecretName: "external-secrets"
            secrets: ["db-password", "api-key"]
        }
    }
]

resources = pl.execute_pipeline(pipeline_steps, None)
```

### Custom Function Registration

```kcl
import .function_registry as fr

custom_fn = lambda input: any -> any {
    {
        apiVersion: "v1"
        kind: "ConfigMap"
        metadata: {
            name: input.name
            namespace: input.namespace
        }
        data: input.data
    }
}

extended_registry = fr.register_function(fr.registry, "create_configmap", custom_fn)

result = fr.execute_function(extended_registry, "create_configmap", {
    name: "app-config"
    namespace: "default"
    data: {"key": "value"}
})
```

### Crossplane Composition with Function Pipeline

```kcl
import .crossplane_function as cf

steps = [
    {
        functionName: "create_namespace"
        input: {
            name: "database-ns"
            labels: {"team": "platform"}
        }
    }
    {
        functionName: "create_xrd"
        input: {
            kind: "Database"
            plural: "databases"
            group: "platform.example.com"
            claimKind: "DatabaseClaim"
            claimPlural: "databaseclaims"
            owner: "platform@example.com"
        }
    }
]

oxr = option("params").oxr
items = [cf.create_crossplane_function(steps, oxr)]
```

## Testing

Run the test files to verify functionality:

```bash
kcl run be/function_registry_test.k
kcl run be/pipeline_test.k
kcl run be/crossplane_function_test.k
```

## Architecture

```
function_registry.k
├── FunctionRegistry schema
├── Built-in functions (create_namespace, create_secret, etc.)
└── Helper functions (register_function, execute_function, list_functions)

pipeline.k
├── PipelineStep schema
├── Pipeline schema
└── Execution functions (execute_pipeline_steps, create_pipeline, execute_pipeline)

crossplane_function.k
├── CrossplaneFunctionConfig schema
├── execute_as_crossplane_function
├── create_crossplane_function
└── create_dynamic_composition
```

## Use Cases

1. **Infrastructure as Code**: Define reusable infrastructure patterns
2. **Crossplane Compositions**: Build dynamic XRD compositions
3. **Multi-Cloud Resources**: Abstract provider-specific resources
4. **GitOps Workflows**: Generate consistent configurations
5. **Policy Enforcement**: Apply organizational standards
