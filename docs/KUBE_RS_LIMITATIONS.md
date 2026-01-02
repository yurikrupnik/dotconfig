# Kube-rs Limitations Map

This document provides a comprehensive overview of features that cannot be implemented in pure kube-rs, including error messages, reasons, and workarounds.

## Table of Contents

1. [Features Not Available in kube-rs](#features-not-available-in-kube-rs)
2. [Common Error Patterns and Solutions](#common-error-patterns-and-solutions)
3. [Workarounds and Alternatives](#workarounds-and-alternatives)
4. [Best Practices](#best-practices)

---

## Features Not Available in kube-rs

### 1. Helm Chart Installation

**Limitation:** kube-rs has no native Helm support - it cannot install, upgrade, or manage Helm charts directly.

**Reason:** Helm is a separate ecosystem with its own templating engine, dependency management, and release tracking. kube-rs focuses on raw Kubernetes API operations.

**Error (if attempted):** N/A - feature doesn't exist

**Workaround:**

```rust
use tokio::process::Command;

pub struct HelmClient {
    helm_binary: String,
}

impl HelmClient {
    pub async fn install_or_upgrade(
        &self,
        name: &str,
        namespace: &str,
        chart: &str,
        values: Option<&serde_json::Value>,
    ) -> Result<(), HelmError> {
        let mut args = vec![
            "upgrade", "--install", name, chart,
            "--namespace", namespace,
            "--create-namespace",
            "--wait",
        ];

        let output = Command::new(&self.helm_binary)
            .args(&args)
            .output()
            .await?;

        if !output.status.success() {
            return Err(HelmError::Execution(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }
        Ok(())
    }
}
```

---

### 2. KCL Execution

**Limitation:** kube-rs cannot execute KCL (KCL Configuration Language) files for manifest generation.

**Reason:** KCL is an external tool with its own runtime. kube-rs only handles Kubernetes API operations.

**Error (if attempted):** N/A - feature doesn't exist

**Workaround:**

```rust
use tokio::process::Command;

pub struct KclExecutor {
    kcl_binary: String,
}

impl KclExecutor {
    pub async fn execute(&self, source: &str) -> Result<Vec<serde_json::Value>, KclError> {
        let output = Command::new(&self.kcl_binary)
            .args(["run", source])
            .output()
            .await?;

        if !output.status.success() {
            return Err(KclError::Execution(
                String::from_utf8_lossy(&output.stderr).to_string()
            ));
        }

        // Parse YAML output
        let yaml = String::from_utf8_lossy(&output.stdout);
        self.parse_manifests(&yaml)
    }
}
```

---

### 3. CRD Schema Auto-Generation

**Limitation:** kube-rs cannot automatically generate complete OpenAPI v3 schemas for CRDs. The `#[derive(CustomResource)]` macro generates Rust types but not full JSON Schema.

**Reason:** JSON Schema generation requires additional dependencies and configuration.

**Error (if missing schemars):**
```
error[E0433]: failed to resolve: use of undeclared crate or module `schemars`
```

**Workaround:** Use `schemars` crate with `#[derive(JsonSchema)]`:

```rust
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "PlatformApp",
    namespaced,
    status = "PlatformAppStatus",
)]
#[serde(rename_all = "camelCase")]  // CRITICAL: Match K8s naming convention
pub struct PlatformAppSpec {
    pub name: String,
    pub installation_type: String,  // Serializes as "installationType"
}
```

---

### 4. Exec into Pods

**Limitation:** Pod exec via kube-rs exists but has limitations with interactive terminals and complex I/O.

**Reason:** WebSocket-based exec requires careful stream handling.

**Error (common issues):**
```
Error: Failed to connect: WebSocket protocol error: Invalid HTTP upgrade header
Error: Stream unexpectedly closed
```

**Workaround:**

```rust
use kube::api::AttachParams;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn exec_command(
    pods: &Api<Pod>,
    name: &str,
    command: Vec<&str>,
) -> Result<String, Error> {
    let attach_params = AttachParams {
        container: Some("main".to_string()),
        stdin: false,
        stdout: true,
        stderr: true,
        tty: false,
        ..Default::default()
    };

    let mut attached = pods.exec(name, command, &attach_params).await?;

    let mut stdout = String::new();
    if let Some(mut stdout_stream) = attached.stdout() {
        stdout_stream.read_to_string(&mut stdout).await?;
    }

    Ok(stdout)
}
```

---

### 5. Port Forwarding

**Limitation:** Port forwarding requires long-lived connections and careful lifecycle management.

**Reason:** The `Api::portforward` method returns a stream that must be maintained for the duration of the forwarding.

**Error (common):**
```
Error: Connection reset by peer
Error: Port forward stream closed unexpectedly
```

**Workaround:**

```rust
use kube::api::Portforwarder;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn port_forward(
    pods: &Api<Pod>,
    name: &str,
    port: u16,
) -> Result<(), Error> {
    let mut pf = pods.portforward(name, &[port]).await?;

    // Get the forwarded port stream
    let mut port_stream = pf.take_stream(port).unwrap();

    // Keep the connection alive in a background task
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            // Send keepalive if needed
        }
    });

    Ok(())
}
```

---

### 6. Real-time Log Streaming

**Limitation:** Log streaming has buffer limitations and can cause memory issues with large logs.

**Reason:** kube-rs buffers log streams in memory by default.

**Error (with large logs):**
```
Error: memory allocation failed
Error: buffer capacity exceeded
```

**Workaround:** Use pagination with `tail_lines`:

```rust
use kube::api::LogParams;
use futures::StreamExt;

async fn stream_logs_paginated(
    pods: &Api<Pod>,
    name: &str,
) -> Result<impl Stream<Item = String>, Error> {
    let params = LogParams {
        follow: true,
        tail_lines: Some(100),  // Limit initial lines
        timestamps: true,
        ..Default::default()
    };

    let stream = pods.log_stream(name, &params).await?;

    Ok(stream.map(|bytes| String::from_utf8_lossy(&bytes).to_string()))
}
```

---

### 7. Server-Side Apply Conflicts

**Limitation:** Field manager conflicts when multiple controllers modify the same resource.

**Reason:** Kubernetes uses field ownership tracking. Conflicts occur when different managers try to modify the same fields.

**Error:**
```
Conflict: Apply failed with 1 conflict: conflict with "other-controller" using apps/v1: .spec.replicas
```

**Workaround:** Use unique field manager names and force when appropriate:

```rust
use kube::api::{Patch, PatchParams};

async fn apply_with_force(
    api: &Api<Deployment>,
    name: &str,
    resource: &Deployment,
) -> Result<(), Error> {
    let patch_params = PatchParams::apply("platform-operator.yurikrupnik.com")
        .force();  // Override conflicts

    api.patch(name, &patch_params, &Patch::Apply(resource)).await?;
    Ok(())
}
```

---

### 8. Dynamic CRD Discovery

**Limitation:** Cannot discover and work with unknown CRD schemas at runtime without compile-time types.

**Reason:** Rust's type system requires known types at compile time.

**Error:**
```
error[E0277]: the trait bound `UnknownCRD: DeserializeOwned` is not satisfied
```

**Workaround:** Use `DynamicObject` for runtime schema discovery:

```rust
use kube::api::DynamicObject;
use kube::discovery::{ApiResource, Discovery};

async fn discover_and_list_crd(
    client: Client,
    group: &str,
    version: &str,
    kind: &str,
) -> Result<Vec<DynamicObject>, Error> {
    let gvk = GroupVersionKind {
        group: group.to_string(),
        version: version.to_string(),
        kind: kind.to_string(),
    };

    let ar = ApiResource::from_gvk(&gvk);
    let api: Api<DynamicObject> = Api::all_with(client, &ar);

    let list = api.list(&Default::default()).await?;
    Ok(list.items)
}
```

---

### 9. Multi-Cluster Management

**Limitation:** A single `kube::Client` is bound to one cluster.

**Reason:** The client uses a specific kubeconfig context.

**Error (if misconfigured):**
```
Error: Failed to connect to cluster: connection refused
Error: Context not found in kubeconfig
```

**Workaround:** Create multiple clients with different configurations:

```rust
use kube::Config;
use std::collections::HashMap;

pub struct MultiClusterManager {
    clients: HashMap<String, Client>,
}

impl MultiClusterManager {
    pub async fn add_cluster(&mut self, name: &str, kubeconfig_path: &str) -> Result<(), Error> {
        let config = Config::from_kubeconfig(&kube::config::KubeConfigOptions {
            context: Some(name.to_string()),
            ..Default::default()
        }).await?;

        let client = Client::try_from(config)?;
        self.clients.insert(name.to_string(), client);
        Ok(())
    }

    pub fn get_client(&self, cluster: &str) -> Option<&Client> {
        self.clients.get(cluster)
    }
}
```

---

### 10. Admission Webhooks

**Limitation:** Running admission webhooks requires TLS certificate management and HTTP server setup.

**Reason:** Kubernetes requires HTTPS for webhooks with proper certificate chains.

**Error (certificate issues):**
```
Error: x509: certificate signed by unknown authority
Error: TLS handshake failed
```

**Workaround:** Use cert-manager for certificate lifecycle:

```rust
use axum::{Router, routing::post};
use axum_server::tls_rustls::RustlsConfig;

async fn start_webhook_server(
    cert_path: &str,
    key_path: &str,
) -> Result<(), Error> {
    let config = RustlsConfig::from_pem_file(cert_path, key_path).await?;

    let app = Router::new()
        .route("/validate", post(validate_handler))
        .route("/mutate", post(mutate_handler));

    axum_server::bind_rustls("0.0.0.0:8443".parse()?, config)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
```

---

## Common Error Patterns and Solutions

### "Failed to infer config"

**Error:**
```
Error: Failed to infer config: MissingKubeContext
```

**Reason:** No kubeconfig or in-cluster config available.

**Solution:**
```rust
let client = Client::try_default().await.map_err(|e| {
    if std::env::var("KUBERNETES_SERVICE_HOST").is_ok() {
        Error::msg("In-cluster config failed. Check ServiceAccount permissions.")
    } else {
        Error::msg(format!("No kubeconfig found: {}", e))
    }
})?;
```

---

### "Forbidden: User cannot list resource"

**Error:**
```
Error: Forbidden: User "system:serviceaccount:default:operator" cannot list resource "pods"
```

**Reason:** RBAC permissions missing.

**Solution:** Apply proper RBAC:
```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: platform-operator
rules:
  - apiGroups: ["platform.yurikrupnik.com"]
    resources: ["platformapps", "gitopsapps"]
    verbs: ["*"]
  - apiGroups: [""]
    resources: ["secrets", "configmaps"]
    verbs: ["get", "list", "watch", "create", "update", "patch"]
```

---

### "the object has been modified"

**Error:**
```
Error: the object has been modified; please apply your changes to the latest version
```

**Reason:** Optimistic concurrency conflict.

**Solution:**
```rust
async fn update_with_retry<T: Resource + Clone + DeserializeOwned + Serialize>(
    api: &Api<T>,
    name: &str,
    mut update_fn: impl FnMut(&mut T),
    max_retries: u32,
) -> Result<T, Error> {
    for attempt in 0..max_retries {
        let mut obj = api.get(name).await?;
        update_fn(&mut obj);

        match api.replace(name, &PostParams::default(), &obj).await {
            Ok(updated) => return Ok(updated),
            Err(kube::Error::Api(e)) if e.code == 409 => {
                // Conflict - retry with backoff
                tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(attempt))).await;
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(Error::msg("Max retries exceeded"))
}
```

---

### "deadline exceeded" during watch

**Error:**
```
Error: deadline exceeded
Error: watch connection closed
```

**Reason:** Network timeout or API server overload.

**Solution:**
```rust
use kube::runtime::watcher::Config;

let watcher_config = Config::default()
    .timeout(300)  // 5 minute timeout
    .any_semantic();  // Continue from bookmark on reconnect

Controller::new(api, watcher_config)
    .run(reconcile, error_policy, context)
    .await;
```

---

## Best Practices

### 1. Always Use Server-Side Apply with Unique Field Manager

```rust
let patch_params = PatchParams::apply("platform-operator.yurikrupnik.com");
api.patch(name, &patch_params, &Patch::Apply(&resource)).await?;
```

### 2. Handle Resource Versions for Updates

```rust
// Get current resource version
let current = api.get(name).await?;
let resource_version = current.metadata.resource_version.clone();

// Set resource version before update
obj.metadata.resource_version = resource_version;
api.replace(name, &PostParams::default(), &obj).await?;
```

### 3. Use Finalizers for Cleanup

```rust
const FINALIZER: &str = "platform.yurikrupnik.com/cleanup";

async fn add_finalizer(api: &Api<PlatformApp>, name: &str) -> Result<(), Error> {
    let patch = json!({
        "metadata": {
            "finalizers": [FINALIZER]
        }
    });
    api.patch(name, &PatchParams::default(), &Patch::Merge(&patch)).await?;
    Ok(())
}

async fn remove_finalizer(api: &Api<PlatformApp>, name: &str) -> Result<(), Error> {
    let patch = json!({
        "metadata": {
            "finalizers": null
        }
    });
    api.patch(name, &PatchParams::default(), &Patch::Merge(&patch)).await?;
    Ok(())
}
```

### 4. Implement Proper Error Policies

```rust
fn error_policy(
    _app: Arc<PlatformApp>,
    error: &Error,
    _ctx: Arc<Context>,
) -> Action {
    match error {
        Error::Transient(_) => Action::requeue(Duration::from_secs(5)),
        Error::Permanent(_) => Action::await_change(),
        _ => Action::requeue(Duration::from_secs(30)),
    }
}
```

### 5. Use Conditions for Status Reporting

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Condition {
    #[serde(rename = "type")]
    pub condition_type: String,
    pub status: String,  // "True", "False", "Unknown"
    pub reason: String,
    pub message: String,
    pub last_transition_time: String,
}

fn create_ready_condition(ready: bool, message: &str) -> Condition {
    Condition {
        condition_type: "Ready".to_string(),
        status: if ready { "True" } else { "False" }.to_string(),
        reason: if ready { "Succeeded" } else { "Failed" }.to_string(),
        message: message.to_string(),
        last_transition_time: chrono::Utc::now().to_rfc3339(),
    }
}
```

---

## Version Compatibility

| Component | Version | Notes |
|-----------|---------|-------|
| kube-rs | 2.0.1 | Latest stable |
| k8s-openapi | 0.26 | `features = ["latest"]` |
| schemars | 0.8 | For JSON Schema generation |
| tokio | 1.47 | Async runtime |
| serde | 1.0 | Serialization |
