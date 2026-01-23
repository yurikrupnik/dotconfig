use crate::operator::{Context, OperatorError, Result};
use crate::operator::types::{
    CrossplaneResource, CrossplaneResourceStatus, CrossplaneCondition, CrossplanePhase,
    CompositeResourceStatus, ManagedResourceStatus, ConnectionDetails,
};
use chrono::Utc;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Reconciles CrossplaneResource resources
pub async fn reconcile(
    resource: Arc<CrossplaneResource>,
    ctx: Arc<Context>,
) -> std::result::Result<Action, OperatorError> {
    let name = resource.name_any();
    let namespace = resource.namespace().unwrap_or_else(|| "default".to_string());

    info!("Reconciling CrossplaneResource {}/{}", namespace, name);

    let api: Api<CrossplaneResource> = Api::namespaced(ctx.client.clone(), &namespace);

    // Update status to Creating
    update_phase(&api, &name, CrossplanePhase::Creating, "Creating Crossplane claim").await?;

    // Create or update the Crossplane claim
    let xr_name = create_crossplane_claim(&resource, &ctx, &namespace).await?;

    // Get managed resources status
    let managed_resources = get_managed_resources_status(&resource, &ctx, &xr_name).await?;

    // Check if all resources are ready
    let all_ready = managed_resources.iter().all(|r| r.ready);
    let phase = if all_ready {
        CrossplanePhase::Ready
    } else {
        CrossplanePhase::Creating
    };

    // Update status
    update_status_full(
        &api,
        &name,
        phase,
        &xr_name,
        managed_resources,
        &resource.spec.write_connection_secret_to_ref,
    ).await?;

    if all_ready {
        info!("CrossplaneResource {}/{} is Ready", namespace, name);
        Ok(Action::requeue(Duration::from_secs(300)))
    } else {
        // Requeue quickly to check the status
        Ok(Action::requeue(Duration::from_secs(30)))
    }
}

async fn create_crossplane_claim(
    resource: &CrossplaneResource,
    ctx: &Context,
    namespace: &str,
) -> Result<String> {
    let composition_name = &resource.spec.composition_ref.name;

    info!(
        "Creating Crossplane claim for composition {} in {}",
        composition_name, namespace
    );

    // If a claim reference is provided, use it; otherwise create an XR directly
    if let Some(claim_ref) = &resource.spec.claim_ref {
        // Create a claim
        let claim = serde_json::json!({
            "apiVersion": claim_ref.api_version,
            "kind": claim_ref.kind,
            "metadata": {
                "name": resource.name_any(),
                "namespace": namespace,
                "labels": resource.spec.labels
            },
            "spec": {
                "compositionRef": {
                    "name": composition_name
                },
                "parameters": resource.spec.parameters,
                "writeConnectionSecretToRef": resource.spec.write_connection_secret_to_ref.as_ref().map(|s| {
                    serde_json::json!({
                        "name": s.name
                    })
                })
            }
        });

        ctx.crossplane_client.apply_resource(&claim).await?;

        Ok(resource.name_any())
    } else {
        // Create an XR (composite resource) directly
        let xr_name = format!("{}-xr", resource.name_any());

        // Get XRD info from composition to determine an API group
        let xr = serde_json::json!({
            "apiVersion": "yurikrupnik.com/v1alpha1",
            "kind": "XDynamicResource",
            "metadata": {
                "name": xr_name,
                "labels": resource.spec.labels
            },
            "spec": {
                "compositionRef": {
                    "name": composition_name
                },
                "parameters": resource.spec.parameters,
                "writeConnectionSecretToRef": resource.spec.write_connection_secret_to_ref.as_ref().map(|s| {
                    serde_json::json!({
                        "name": s.name,
                        "namespace": s.namespace.as_deref().unwrap_or(namespace)
                    })
                })
            }
        });

        ctx.crossplane_client.apply_resource(&xr).await?;

        Ok(xr_name)
    }
}

async fn get_managed_resources_status(
    _resource: &CrossplaneResource,
    ctx: &Context,
    xr_name: &str,
) -> Result<Vec<ManagedResourceStatus>> {
    // Query Crossplane for managed resources
    // This is a simplified version - in production, you'd query the XR status
    let managed = ctx.crossplane_client.get_managed_resources(xr_name).await?;

    Ok(managed)
}

async fn update_phase(
    api: &Api<CrossplaneResource>,
    name: &str,
    phase: CrossplanePhase,
    message: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = CrossplaneResourceStatus {
        phase,
        last_reconcile_time: Some(now.clone()),
        conditions: vec![CrossplaneCondition {
            condition_type: "Reconciling".to_string(),
            status: "True".to_string(),
            reason: "Reconciling".to_string(),
            message: message.to_string(),
            last_transition_time: now,
        }],
        ..Default::default()
    };

    let patch = serde_json::json!({
        "status": status
    });

    api.patch_status(name, &PatchParams::apply("platform-operator"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

async fn update_status_full(
    api: &Api<CrossplaneResource>,
    name: &str,
    phase: CrossplanePhase,
    xr_name: &str,
    managed_resources: Vec<ManagedResourceStatus>,
    connection_secret: &Option<crate::operator::types::ConnectionSecretRef>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let all_ready = managed_resources.iter().all(|r| r.ready);
    let all_synced = managed_resources.iter().all(|r| r.synced);

    let status = CrossplaneResourceStatus {
        phase,
        composite_resource: Some(CompositeResourceStatus {
            name: xr_name.to_string(),
            ready: all_ready,
            synced: all_synced,
            composition_revision: None,
        }),
        managed_resources,
        connection_details: connection_secret.as_ref().map(|s| ConnectionDetails {
            secret_name: s.name.clone(),
            secret_namespace: s.namespace.clone().unwrap_or_else(|| "default".to_string()),
            available_keys: vec![],
        }),
        last_reconcile_time: Some(now.clone()),
        conditions: vec![CrossplaneCondition {
            condition_type: if all_ready { "Ready" } else { "Synced" }.to_string(),
            status: if all_ready { "True" } else { "False" }.to_string(),
            reason: if all_ready { "Available" } else { "Progressing" }.to_string(),
            message: if all_ready {
                "All managed resources are ready".to_string()
            } else {
                "Waiting for managed resources to be ready".to_string()
            },
            last_transition_time: now,
        }],
        ..Default::default()
    };

    let patch = serde_json::json!({
        "status": status
    });

    api.patch_status(name, &PatchParams::apply("platform-operator"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

/// Error policy for the controller
pub fn error_policy(
    resource: Arc<CrossplaneResource>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "Reconcile error for CrossplaneResource {}: {}",
        resource.name_any(),
        error
    );

    Action::requeue(Duration::from_secs(30))
}
