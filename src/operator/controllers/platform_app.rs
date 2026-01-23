use crate::operator::{Context, OperatorError, Result};
use crate::operator::types::{
    HelmReleaseStatus, KclOutputStatus, PlatformApp, PlatformAppPhase, PlatformAppStatus,
    Condition, InstallationType, ResourceReference,
};
use chrono::Utc;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn, error};

/// Reconciles PlatformApp resources
pub async fn reconcile(
    app: Arc<PlatformApp>,
    ctx: Arc<Context>,
) -> std::result::Result<Action, OperatorError> {
    let name = app.name_any();
    let namespace = app.namespace().unwrap_or_else(|| "default".to_string());

    info!("Reconciling PlatformApp {}/{}", namespace, name);

    let api: Api<PlatformApp> = Api::namespaced(ctx.client.clone(), &namespace);

    // Update status to Installing
    update_phase(&api, &name, PlatformAppPhase::Installing, None).await?;

    // Get target namespace for resources
    let target_ns = app.spec.namespace.as_deref().unwrap_or(&namespace);

    // Install based on an installation type
    let result: Result<(Option<HelmReleaseStatus>, Option<KclOutputStatus>)> = match &app.spec.installation_type {
        InstallationType::Helm => {
            install_helm(&app, &ctx, target_ns).await
        }
        InstallationType::Kcl => {
            install_kcl(&app, &ctx, target_ns).await
        }
        InstallationType::Cue => {
            install_kcl(&app, &ctx, target_ns).await
        }
        InstallationType::All => {
            // Install Helm first (infrastructure), then KCL (customizations)
            let helm_result = install_helm(&app, &ctx, target_ns).await?;
            let kcl_result = install_kcl(&app, &ctx, target_ns).await?;
            Ok((helm_result.0, kcl_result.1))
        }
    };

    match result {
        Ok((helm_status, kcl_status)) => {
            // Update status to Ready
            update_status_full(&api, &name, PlatformAppPhase::Ready, helm_status, kcl_status).await?;
            info!("PlatformApp {}/{} is Ready", namespace, name);
        }
        Err(e) => {
            // Update status to Failed
            update_phase(&api, &name, PlatformAppPhase::Failed, Some(e.to_string())).await?;
            error!("PlatformApp {}/{} failed: {}", namespace, name, e);
            return Err(e);
        }
    }

    // Requeue after interval for drift detection
    Ok(Action::requeue(Duration::from_secs(300)))
}

async fn install_helm(
    app: &PlatformApp,
    ctx: &Context,
    namespace: &str,
) -> Result<(Option<HelmReleaseStatus>, Option<KclOutputStatus>)> {
    if let Some(helm_spec) = &app.spec.helm {
        info!("Installing Helm chart {} for {}", helm_spec.chart, app.name_any());

        let status = ctx.helm_client.install_or_upgrade(
            &app.name_any(),
            namespace,
            helm_spec,
        ).await?;

        Ok((Some(status), None))
    } else {
        Ok((None, None))
    }
}

async fn install_kcl(
    app: &PlatformApp,
    ctx: &Context,
    namespace: &str,
) -> Result<(Option<HelmReleaseStatus>, Option<KclOutputStatus>)> {
    if let Some(kcl_spec) = &app.spec.kcl {
        info!("Applying KCL manifests from {} for {}", kcl_spec.source, app.name_any());

        let result = ctx.kcl_executor.execute(kcl_spec).await?;

        // Apply generated manifests
        let mut resources = Vec::new();
        for manifest in &result.manifests {
            let applied = apply_manifest(&ctx.client, manifest, namespace).await?;
            resources.push(applied);
        }

        let status = KclOutputStatus {
            resources_created: resources.len() as i32,
            last_applied: Some(Utc::now().to_rfc3339()),
            resources,
        };

        Ok((None, Some(status)))
    } else {
        Ok((None, None))
    }
}

async fn apply_manifest(
    client: &kube::Client,
    manifest: &serde_json::Value,
    default_namespace: &str,
) -> Result<ResourceReference> {
    let api_version = manifest.get("apiVersion")
        .and_then(|v| v.as_str())
        .ok_or_else(|| OperatorError::Config("Missing apiVersion".into()))?;

    let kind = manifest.get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| OperatorError::Config("Missing kind".into()))?;

    let name = manifest.get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(|n| n.as_str())
        .ok_or_else(|| OperatorError::Config("Missing metadata.name".into()))?;

    let namespace = manifest.get("metadata")
        .and_then(|m| m.get("namespace"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());

    // Parse API version into group and version
    let (group, version) = parse_api_version(api_version)?;

    let gvk = kube::api::GroupVersionKind {
        group: group.to_string(),
        version: version.to_string(),
        kind: kind.to_string(),
    };

    let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);

    info!("Applying {}/{} in namespace {:?}", kind, name, namespace);

    // Apply using server-side apply
    let api: Api<kube::api::DynamicObject> = if let Some(ns) = &namespace {
        Api::namespaced_with(client.clone(), ns, &api_resource)
    } else {
        Api::namespaced_with(client.clone(), default_namespace, &api_resource)
    };

    let patch_params = PatchParams::apply("platform-operator.yurikrupnik.com");
    let obj: kube::api::DynamicObject = serde_json::from_value(manifest.clone())?;

    api.patch(name, &patch_params, &Patch::Apply(&obj)).await?;

    Ok(ResourceReference {
        api_version: api_version.to_string(),
        kind: kind.to_string(),
        name: name.to_string(),
        namespace,
    })
}

fn parse_api_version(api_version: &str) -> Result<(&str, &str)> {
    let parts: Vec<&str> = api_version.split('/').collect();
    match parts.len() {
        1 => Ok(("", parts[0])),
        2 => Ok((parts[0], parts[1])),
        _ => Err(OperatorError::Config(format!("Invalid apiVersion: {}", api_version))),
    }
}

async fn update_phase(
    api: &Api<PlatformApp>,
    name: &str,
    phase: PlatformAppPhase,
    message: Option<String>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = PlatformAppStatus {
        phase,
        message,
        last_reconcile_time: Some(now.clone()),
        conditions: vec![Condition {
            condition_type: "Reconciling".to_string(),
            status: "True".to_string(),
            reason: "Reconciling".to_string(),
            message: "Resource is being reconciled".to_string(),
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
    api: &Api<PlatformApp>,
    name: &str,
    phase: PlatformAppPhase,
    helm_status: Option<HelmReleaseStatus>,
    kcl_status: Option<KclOutputStatus>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = PlatformAppStatus {
        phase,
        message: None,
        helm_release: helm_status,
        kcl_output: kcl_status,
        last_reconcile_time: Some(now.clone()),
        conditions: vec![Condition {
            condition_type: "Ready".to_string(),
            status: "True".to_string(),
            reason: "Succeeded".to_string(),
            message: "Resource has been successfully reconciled".to_string(),
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
    app: Arc<PlatformApp>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "Reconcile error for PlatformApp {}: {}",
        app.name_any(),
        error
    );

    // Exponential backoff for retries
    Action::requeue(Duration::from_secs(30))
}
