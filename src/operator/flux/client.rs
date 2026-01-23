use crate::operator::{OperatorError, Result};
use kube::api::{Patch, PatchParams};
use kube::{Api, Client};
use tracing::{debug, info};

/// FluxCD client for managing GitOps resources
pub struct FluxClient {
    client: Client,
}

impl FluxClient {
    /// Create a new FluxCD client
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Apply a FluxCD resource (GitRepository, Kustomization, HelmRelease)
    pub async fn apply_resource(&self, resource: &serde_json::Value) -> Result<()> {
        let api_version = resource.get("apiVersion")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OperatorError::Config("Missing apiVersion".into()))?;

        let kind = resource.get("kind")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OperatorError::Config("Missing kind".into()))?;

        let name = resource.get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(|n| n.as_str())
            .ok_or_else(|| OperatorError::Config("Missing metadata.name".into()))?;

        let namespace = resource.get("metadata")
            .and_then(|m| m.get("namespace"))
            .and_then(|n| n.as_str())
            .unwrap_or("default");

        info!("Applying FluxCD {} {}/{}", kind, namespace, name);

        // Use dynamic API to apply the resource
        let gvk = parse_api_version(api_version, kind)?;
        let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);

        let api: Api<kube::api::DynamicObject> = Api::namespaced_with(
            self.client.clone(),
            namespace,
            &api_resource,
        );

        let patch_params = PatchParams::apply("platform-operator.yurikrupnik.com");
        let obj: kube::api::DynamicObject = serde_json::from_value(resource.clone())?;

        api.patch(name, &patch_params, &Patch::Apply(&obj)).await?;

        debug!("Successfully applied FluxCD {} {}/{}", kind, namespace, name);

        Ok(())
    }

    /// Delete a FluxCD resource
    pub async fn delete_resource(
        &self,
        api_version: &str,
        kind: &str,
        name: &str,
        namespace: &str,
    ) -> Result<()> {
        info!("Deleting FluxCD {} {}/{}", kind, namespace, name);

        let gvk = parse_api_version(api_version, kind)?;
        let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);

        let api: Api<kube::api::DynamicObject> = Api::namespaced_with(
            self.client.clone(),
            namespace,
            &api_resource,
        );

        match api.delete(name, &Default::default()).await {
            Ok(_) => {
                debug!("Successfully deleted FluxCD {} {}/{}", kind, namespace, name);
                Ok(())
            }
            Err(kube::Error::Api(e)) if e.code == 404 => {
                debug!("FluxCD {} {}/{} not found, skipping delete", kind, namespace, name);
                Ok(())
            }
            Err(e) => Err(OperatorError::Flux(format!("Failed to delete resource: {}", e))),
        }
    }

    /// Get the status of a FluxCD resource
    pub async fn get_status(
        &self,
        api_version: &str,
        kind: &str,
        name: &str,
        namespace: &str,
    ) -> Result<Option<FluxResourceStatus>> {
        let gvk = parse_api_version(api_version, kind)?;
        let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);

        let api: Api<kube::api::DynamicObject> = Api::namespaced_with(
            self.client.clone(),
            namespace,
            &api_resource,
        );

        match api.get(name).await {
            Ok(obj) => {
                let status = obj.data.get("status").cloned();
                Ok(Some(parse_flux_status(status)?))
            }
            Err(kube::Error::Api(e)) if e.code == 404 => Ok(None),
            Err(e) => Err(OperatorError::Flux(format!("Failed to get resource: {}", e))),
        }
    }

    /// Trigger a reconciliation by annotating the resource
    pub async fn reconcile(
        &self,
        api_version: &str,
        kind: &str,
        name: &str,
        namespace: &str,
    ) -> Result<()> {
        info!("Triggering reconciliation for FluxCD {} {}/{}", kind, namespace, name);

        let gvk = parse_api_version(api_version, kind)?;
        let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);

        let api: Api<kube::api::DynamicObject> = Api::namespaced_with(
            self.client.clone(),
            namespace,
            &api_resource,
        );

        let patch = serde_json::json!({
            "metadata": {
                "annotations": {
                    "reconcile.fluxcd.io/requestedAt": chrono::Utc::now().to_rfc3339()
                }
            }
        });

        api.patch(name, &PatchParams::apply("platform-operator"), &Patch::Merge(&patch))
            .await?;

        Ok(())
    }

    /// Suspend or resume a FluxCD resource
    pub async fn set_suspend(
        &self,
        api_version: &str,
        kind: &str,
        name: &str,
        namespace: &str,
        suspend: bool,
    ) -> Result<()> {
        info!(
            "{} FluxCD {} {}/{}",
            if suspend { "Suspending" } else { "Resuming" },
            kind,
            namespace,
            name
        );

        let gvk = parse_api_version(api_version, kind)?;
        let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);

        let api: Api<kube::api::DynamicObject> = Api::namespaced_with(
            self.client.clone(),
            namespace,
            &api_resource,
        );

        let patch = serde_json::json!({
            "spec": {
                "suspend": suspend
            }
        });

        api.patch(name, &PatchParams::apply("platform-operator"), &Patch::Merge(&patch))
            .await?;

        Ok(())
    }
}

/// Parsed FluxCD resource status
#[derive(Debug, Clone)]
pub struct FluxResourceStatus {
    pub ready: bool,
    pub last_applied_revision: Option<String>,
    pub last_attempted_revision: Option<String>,
    pub message: Option<String>,
}

fn parse_api_version(api_version: &str, kind: &str) -> Result<kube::api::GroupVersionKind> {
    let parts: Vec<&str> = api_version.split('/').collect();

    let (group, version) = match parts.len() {
        1 => ("", parts[0]),
        2 => (parts[0], parts[1]),
        _ => return Err(OperatorError::Config(format!("Invalid apiVersion: {}", api_version))),
    };

    Ok(kube::api::GroupVersionKind {
        group: group.to_string(),
        version: version.to_string(),
        kind: kind.to_string(),
    })
}

fn parse_flux_status(status: Option<serde_json::Value>) -> Result<FluxResourceStatus> {
    let status = status.unwrap_or(serde_json::json!({}));

    // Check conditions for Ready status
    let ready = status.get("conditions")
        .and_then(|c| c.as_array())
        .map(|conditions| {
            conditions.iter().any(|c| {
                c.get("type").and_then(|t| t.as_str()) == Some("Ready") &&
                c.get("status").and_then(|s| s.as_str()) == Some("True")
            })
        })
        .unwrap_or(false);

    let last_applied_revision = status.get("lastAppliedRevision")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let last_attempted_revision = status.get("lastAttemptedRevision")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let message = status.get("conditions")
        .and_then(|c| c.as_array())
        .and_then(|conditions| {
            conditions.iter()
                .find(|c| c.get("type").and_then(|t| t.as_str()) == Some("Ready"))
                .and_then(|c| c.get("message"))
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
        });

    Ok(FluxResourceStatus {
        ready,
        last_applied_revision,
        last_attempted_revision,
        message,
    })
}
