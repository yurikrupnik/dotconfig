use crate::operator::dependencies::known_dependencies;
use crate::operator::{Context, OperatorError, Result};
use crate::operator::types::{
    GitOpsApp, GitOpsAppStatus, GitOpsCondition, GitOpsPhase,
};
use chrono::Utc;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Reconciles GitOpsApp resources
pub async fn reconcile(
    app: Arc<GitOpsApp>,
    ctx: Arc<Context>,
) -> std::result::Result<Action, OperatorError> {
    let name = app.name_any();
    let namespace = app.namespace().unwrap_or_else(|| "default".to_string());

    info!("Reconciling GitOpsApp {}/{}", namespace, name);

    let api: Api<GitOpsApp> = Api::namespaced(ctx.client.clone(), &namespace);

    // Check dependencies before proceeding
    let deps = known_dependencies::gitops_app_deps();
    let missing = ctx.get_missing_dependencies(&deps).await;

    if !missing.is_empty() {
        let missing_names: Vec<_> = missing
            .iter()
            .map(|r| {
                let hint = r.dependency.install_hint.as_deref().unwrap_or("See documentation");
                format!("{} ({})", r.dependency.name, hint)
            })
            .collect();

        let message = format!(
            "Missing required dependencies: {}",
            missing_names.join(", ")
        );

        warn!("GitOpsApp {}/{}: {}", namespace, name, message);

        update_phase_with_condition(
            &api,
            &name,
            GitOpsPhase::Blocked,
            "DependencyMissing",
            &message,
        )
        .await?;

        // Requeue to check again later
        return Ok(Action::requeue(Duration::from_secs(60)));
    }

    // Check if suspended
    if app.spec.suspend {
        update_phase(&api, &name, GitOpsPhase::Suspended, "Reconciliation suspended").await?;
        return Ok(Action::await_change());
    }

    // Update status to Syncing
    update_phase(&api, &name, GitOpsPhase::Syncing, "Creating FluxCD resources").await?;

    // Create FluxCD GitRepository
    let git_repo_name = create_git_repository(&app, &ctx, &namespace).await?;

    // Create FluxCD Kustomization or HelmRelease based on source_type
    let flux_resource_name = match &app.spec.source_type {
        crate::operator::types::GitOpsSourceType::Kustomization => {
            create_kustomization(&app, &ctx, &namespace, &git_repo_name).await?
        }
        crate::operator::types::GitOpsSourceType::Helm => {
            create_helm_release(&app, &ctx, &namespace, &git_repo_name).await?
        }
    };

    // Update status to Synced
    update_status_full(
        &api,
        &name,
        GitOpsPhase::Synced,
        Some(git_repo_name),
        Some(flux_resource_name),
    ).await?;

    info!("GitOpsApp {}/{} is Synced", namespace, name);

    // Requeue to check sync status
    Ok(Action::requeue(Duration::from_secs(60)))
}

async fn create_git_repository(
    app: &GitOpsApp,
    ctx: &Context,
    namespace: &str,
) -> Result<String> {
    let name = format!("{}-gitrepo", app.name_any());
    let git_spec = &app.spec.git_repository;

    info!("Creating GitRepository {} for {}", name, app.name_any());

    let git_repo = serde_json::json!({
        "apiVersion": "source.toolkit.fluxcd.io/v1",
        "kind": "GitRepository",
        "metadata": {
            "name": name,
            "namespace": namespace
        },
        "spec": {
            "interval": git_spec.interval,
            "url": git_spec.url,
            "ref": {
                "branch": git_spec.branch
            },
            "secretRef": git_spec.secret_ref.as_ref().map(|s| serde_json::json!({
                "name": s.name
            }))
        }
    });

    ctx.flux_client.apply_resource(&git_repo).await?;

    Ok(name)
}

async fn create_kustomization(
    app: &GitOpsApp,
    ctx: &Context,
    namespace: &str,
    git_repo_name: &str,
) -> Result<String> {
    let name = format!("{}-ks", app.name_any());
    let target_ns = app.spec.target_namespace.as_deref().unwrap_or(namespace);

    info!("Creating Kustomization {} for {}", name, app.name_any());

    let kustomization = serde_json::json!({
        "apiVersion": "kustomize.toolkit.fluxcd.io/v1",
        "kind": "Kustomization",
        "metadata": {
            "name": name,
            "namespace": namespace
        },
        "spec": {
            "interval": app.spec.interval,
            "path": app.spec.path,
            "prune": app.spec.prune,
            "sourceRef": {
                "kind": "GitRepository",
                "name": git_repo_name
            },
            "targetNamespace": target_ns,
            "dependsOn": app.spec.depends_on.as_ref().map(|deps| {
                deps.iter().map(|d| serde_json::json!({
                    "name": format!("{}-ks", d.name),
                    "namespace": d.namespace.as_deref().unwrap_or(namespace)
                })).collect::<Vec<_>>()
            })
        }
    });

    ctx.flux_client.apply_resource(&kustomization).await?;

    Ok(name)
}

async fn create_helm_release(
    app: &GitOpsApp,
    ctx: &Context,
    namespace: &str,
    git_repo_name: &str,
) -> Result<String> {
    let name = format!("{}-hr", app.name_any());
    let target_ns = app.spec.target_namespace.as_deref().unwrap_or(namespace);
    let helm_spec = app.spec.helm.as_ref()
        .ok_or_else(|| OperatorError::Config("Helm spec required for Helm source type".into()))?;

    info!("Creating HelmRelease {} for {}", name, app.name_any());

    let helm_release = serde_json::json!({
        "apiVersion": "helm.toolkit.fluxcd.io/v2",
        "kind": "HelmRelease",
        "metadata": {
            "name": name,
            "namespace": namespace
        },
        "spec": {
            "interval": app.spec.interval,
            "chart": {
                "spec": {
                    "chart": helm_spec.chart,
                    "sourceRef": {
                        "kind": "GitRepository",
                        "name": git_repo_name
                    }
                }
            },
            "targetNamespace": target_ns,
            "values": helm_spec.values,
            "releaseName": helm_spec.release_name.as_deref().unwrap_or(&app.name_any())
        }
    });

    ctx.flux_client.apply_resource(&helm_release).await?;

    Ok(name)
}

async fn update_phase(
    api: &Api<GitOpsApp>,
    name: &str,
    phase: GitOpsPhase,
    message: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = GitOpsAppStatus {
        phase,
        conditions: vec![GitOpsCondition {
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

/// Update phase with a specific condition (e.g., for dependency issues)
async fn update_phase_with_condition(
    api: &Api<GitOpsApp>,
    name: &str,
    phase: GitOpsPhase,
    reason: &str,
    message: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = GitOpsAppStatus {
        phase,
        conditions: vec![GitOpsCondition {
            condition_type: "Ready".to_string(),
            status: "False".to_string(),
            reason: reason.to_string(),
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
    api: &Api<GitOpsApp>,
    name: &str,
    phase: GitOpsPhase,
    git_repository_name: Option<String>,
    flux_resource_name: Option<String>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = GitOpsAppStatus {
        phase,
        last_sync_time: Some(now.clone()),
        git_repository_name,
        flux_resource_name,
        conditions: vec![GitOpsCondition {
            condition_type: "Ready".to_string(),
            status: "True".to_string(),
            reason: "Succeeded".to_string(),
            message: "FluxCD resources created successfully".to_string(),
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
    app: Arc<GitOpsApp>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "Reconcile error for GitOpsApp {}: {}",
        app.name_any(),
        error
    );

    Action::requeue(Duration::from_secs(30))
}
