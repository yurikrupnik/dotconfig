use crate::operator::types::ManagedResourceStatus;
use crate::operator::{OperatorError, Result};
use kube::api::{Patch, PatchParams};
use kube::{Api, Client};
use tracing::{debug, info};

/// Crossplane client for managing composite resources and claims
pub struct CrossplaneClient {
    client: Client,
}

impl CrossplaneClient {
    /// Create a new Crossplane client
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Apply a Crossplane resource (XR or Claim)
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
            .and_then(|n| n.as_str());

        info!("Applying Crossplane {} {}", kind, name);

        let gvk = parse_api_version(api_version, kind)?;
        let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);

        let api: Api<kube::api::DynamicObject> = if let Some(ns) = namespace {
            Api::namespaced_with(self.client.clone(), ns, &api_resource)
        } else {
            Api::all_with(self.client.clone(), &api_resource)
        };

        let patch_params = PatchParams::apply("platform-operator.yurikrupnik.com");
        let obj: kube::api::DynamicObject = serde_json::from_value(resource.clone())?;

        api.patch(name, &patch_params, &Patch::Apply(&obj)).await?;

        debug!("Successfully applied Crossplane {} {}", kind, name);

        Ok(())
    }

    /// Delete a Crossplane resource
    pub async fn delete_resource(
        &self,
        api_version: &str,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> Result<()> {
        info!("Deleting Crossplane {} {}", kind, name);

        let gvk = parse_api_version(api_version, kind)?;
        let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);

        let api: Api<kube::api::DynamicObject> = if let Some(ns) = namespace {
            Api::namespaced_with(self.client.clone(), ns, &api_resource)
        } else {
            Api::all_with(self.client.clone(), &api_resource)
        };

        match api.delete(name, &Default::default()).await {
            Ok(_) => {
                debug!("Successfully deleted Crossplane {} {}", kind, name);
                Ok(())
            }
            Err(kube::Error::Api(e)) if e.code == 404 => {
                debug!("Crossplane {} {} not found, skipping delete", kind, name);
                Ok(())
            }
            Err(e) => Err(OperatorError::Crossplane(format!("Failed to delete resource: {}", e))),
        }
    }

    /// Get managed resources for a composite resource
    pub async fn get_managed_resources(&self, xr_name: &str) -> Result<Vec<ManagedResourceStatus>> {
        // In a real implementation, we would:
        // 1. Get the XR and read its status.resources
        // 2. Query each managed resource for its status
        // For now, return an empty list - the actual implementation would require
        // knowing the XR's API group which varies by composition

        debug!("Getting managed resources for XR {}", xr_name);

        // Placeholder - in production this would query the actual resources
        Ok(vec![])
    }

    /// Get the status of a composite resource
    pub async fn get_xr_status(
        &self,
        api_version: &str,
        kind: &str,
        name: &str,
    ) -> Result<Option<CrossplaneXRStatus>> {
        let gvk = parse_api_version(api_version, kind)?;
        let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);

        let api: Api<kube::api::DynamicObject> = Api::all_with(
            self.client.clone(),
            &api_resource,
        );

        match api.get(name).await {
            Ok(obj) => {
                let status = obj.data.get("status").cloned();
                Ok(Some(parse_xr_status(status)?))
            }
            Err(kube::Error::Api(e)) if e.code == 404 => Ok(None),
            Err(e) => Err(OperatorError::Crossplane(format!("Failed to get XR: {}", e))),
        }
    }

    /// Get the status of a claim
    pub async fn get_claim_status(
        &self,
        api_version: &str,
        kind: &str,
        name: &str,
        namespace: &str,
    ) -> Result<Option<CrossplaneClaimStatus>> {
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
                Ok(Some(parse_claim_status(status)?))
            }
            Err(kube::Error::Api(e)) if e.code == 404 => Ok(None),
            Err(e) => Err(OperatorError::Crossplane(format!("Failed to get claim: {}", e))),
        }
    }
}

/// Parsed Crossplane XR status
#[derive(Debug, Clone)]
pub struct CrossplaneXRStatus {
    pub ready: bool,
    pub synced: bool,
    pub composition_ref: Option<String>,
    pub connection_secret_ref: Option<String>,
}

/// Parsed Crossplane Claim status
#[derive(Debug, Clone)]
pub struct CrossplaneClaimStatus {
    pub ready: bool,
    pub synced: bool,
    pub resource_ref: Option<String>,
    pub connection_secret_ref: Option<String>,
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

fn parse_xr_status(status: Option<serde_json::Value>) -> Result<CrossplaneXRStatus> {
    let status = status.unwrap_or(serde_json::json!({}));

    let ready = status.get("conditions")
        .and_then(|c| c.as_array())
        .map(|conditions| {
            conditions.iter().any(|c| {
                c.get("type").and_then(|t| t.as_str()) == Some("Ready") &&
                c.get("status").and_then(|s| s.as_str()) == Some("True")
            })
        })
        .unwrap_or(false);

    let synced = status.get("conditions")
        .and_then(|c| c.as_array())
        .map(|conditions| {
            conditions.iter().any(|c| {
                c.get("type").and_then(|t| t.as_str()) == Some("Synced") &&
                c.get("status").and_then(|s| s.as_str()) == Some("True")
            })
        })
        .unwrap_or(false);

    let composition_ref = status.get("compositionRef")
        .and_then(|c| c.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());

    let connection_secret_ref = status.get("connectionDetails")
        .and_then(|c| c.get("lastPublishedTime"))
        .and_then(|t| t.as_str())
        .map(|_| "published".to_string());

    Ok(CrossplaneXRStatus {
        ready,
        synced,
        composition_ref,
        connection_secret_ref,
    })
}

fn parse_claim_status(status: Option<serde_json::Value>) -> Result<CrossplaneClaimStatus> {
    let status = status.unwrap_or(serde_json::json!({}));

    let ready = status.get("conditions")
        .and_then(|c| c.as_array())
        .map(|conditions| {
            conditions.iter().any(|c| {
                c.get("type").and_then(|t| t.as_str()) == Some("Ready") &&
                c.get("status").and_then(|s| s.as_str()) == Some("True")
            })
        })
        .unwrap_or(false);

    let synced = status.get("conditions")
        .and_then(|c| c.as_array())
        .map(|conditions| {
            conditions.iter().any(|c| {
                c.get("type").and_then(|t| t.as_str()) == Some("Synced") &&
                c.get("status").and_then(|s| s.as_str()) == Some("True")
            })
        })
        .unwrap_or(false);

    let resource_ref = status.get("resourceRef")
        .and_then(|r| r.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());

    let connection_secret_ref = status.get("connectionDetails")
        .and_then(|c| c.get("lastPublishedTime"))
        .and_then(|t| t.as_str())
        .map(|_| "published".to_string());

    Ok(CrossplaneClaimStatus {
        ready,
        synced,
        resource_ref,
        connection_secret_ref,
    })
}
