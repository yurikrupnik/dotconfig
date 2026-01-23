//! Google Workspace Sync Controller
//!
//! Syncs users and groups from Google Workspace to Kubernetes CRDs.

use crate::operator::google_workspace::{GoogleWorkspaceClient, ServiceAccountKey};
use crate::operator::types::{
    GoogleWorkspaceConfig, GoogleWorkspaceConfigStatus, GoogleWorkspaceGroup,
    GoogleWorkspaceGroupSpec, GoogleWorkspaceGroupStatus, GoogleWorkspacePhase,
    GoogleWorkspaceUser, GoogleWorkspaceUserSpec, GoogleWorkspaceUserStatus,
    GroupMemberRef, GoogleWorkspaceCondition,
};
use crate::operator::{Context, OperatorError, Result};
use chrono::Utc;
use k8s_openapi::api::core::v1::Secret;
use kube::api::{ObjectMeta, Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Reconciles GoogleWorkspaceConfig resources
pub async fn reconcile(
    config: Arc<GoogleWorkspaceConfig>,
    ctx: Arc<Context>,
) -> std::result::Result<Action, OperatorError> {
    let name = config.name_any();

    info!("Reconciling GoogleWorkspaceConfig: {}", name);

    let api: Api<GoogleWorkspaceConfig> = Api::all(ctx.client.clone());

    // Update status to Initializing
    update_phase(&api, &name, GoogleWorkspacePhase::Initializing, "Initializing Google Workspace client").await?;

    // Load service account credentials from secret
    let key = load_service_account_key(&ctx.client, &config).await?;

    // Create Google Workspace client
    let gw_client = GoogleWorkspaceClient::new(
        &key,
        config.spec.auth.admin_email.clone(),
        config.spec.domain.clone(),
        config.spec.customer_id.clone(),
        config.spec.auth.scopes.clone(),
    )?;

    // Update status to Syncing
    update_phase(&api, &name, GoogleWorkspacePhase::Syncing, "Syncing users and groups").await?;

    let mut users_synced = 0u32;
    let mut groups_synced = 0u32;

    // Sync users if enabled
    if config.spec.sync.sync_users {
        match sync_users(&ctx.client, &config, &gw_client).await {
            Ok(count) => {
                users_synced = count;
                info!("Synced {} users for config {}", count, name);
            }
            Err(e) => {
                error!("Failed to sync users: {}", e);
                update_phase(&api, &name, GoogleWorkspacePhase::Failed, &format!("Failed to sync users: {}", e)).await?;
                return Ok(Action::requeue(Duration::from_secs(60)));
            }
        }
    }

    // Sync groups if enabled
    if config.spec.sync.sync_groups {
        match sync_groups(&ctx.client, &config, &gw_client).await {
            Ok(count) => {
                groups_synced = count;
                info!("Synced {} groups for config {}", count, name);
            }
            Err(e) => {
                error!("Failed to sync groups: {}", e);
                update_phase(&api, &name, GoogleWorkspacePhase::Failed, &format!("Failed to sync groups: {}", e)).await?;
                return Ok(Action::requeue(Duration::from_secs(60)));
            }
        }
    }

    // Update user group memberships
    if config.spec.sync.sync_users && config.spec.sync.sync_groups {
        if let Err(e) = update_user_group_memberships(&ctx.client, &config, &gw_client).await {
            warn!("Failed to update user group memberships: {}", e);
        }
    }

    // Update status to Ready
    update_status_full(&api, &name, users_synced, groups_synced).await?;

    info!("GoogleWorkspaceConfig {} is Ready", name);

    // Parse sync interval
    let interval = parse_duration(&config.spec.sync.interval).unwrap_or(Duration::from_secs(300));
    Ok(Action::requeue(interval))
}

/// Load service account key from Kubernetes secret
async fn load_service_account_key(
    client: &kube::Client,
    config: &GoogleWorkspaceConfig,
) -> Result<ServiceAccountKey> {
    let secret_ref = &config.spec.auth.service_account_key_ref;
    let namespace = secret_ref.namespace.as_deref().unwrap_or("platform-system");

    let secrets: Api<Secret> = Api::namespaced(client.clone(), namespace);
    let secret = secrets
        .get(&secret_ref.name)
        .await
        .map_err(|e| OperatorError::GoogleWorkspace(format!(
            "Failed to get secret {}/{}: {}", namespace, secret_ref.name, e
        )))?;

    let data = secret.data.ok_or_else(|| {
        OperatorError::GoogleWorkspace(format!(
            "Secret {}/{} has no data", namespace, secret_ref.name
        ))
    })?;

    let key_data = data.get(&secret_ref.key).ok_or_else(|| {
        OperatorError::GoogleWorkspace(format!(
            "Secret {}/{} has no key '{}'", namespace, secret_ref.name, secret_ref.key
        ))
    })?;

    let key_json = String::from_utf8(key_data.0.clone()).map_err(|e| {
        OperatorError::GoogleWorkspace(format!("Invalid UTF-8 in service account key: {}", e))
    })?;

    serde_json::from_str(&key_json).map_err(|e| {
        OperatorError::GoogleWorkspace(format!("Invalid service account key JSON: {}", e))
    })
}

/// Sync users from Google Workspace
async fn sync_users(
    client: &kube::Client,
    config: &GoogleWorkspaceConfig,
    gw_client: &GoogleWorkspaceClient,
) -> Result<u32> {
    let users = gw_client.list_users().await?;
    let namespace = &config.spec.sync.target_namespace;
    let api: Api<GoogleWorkspaceUser> = Api::namespaced(client.clone(), namespace);

    let mut count = 0u32;

    for user in users {
        let name = sanitize_name(&user.primary_email);

        let gw_user = GoogleWorkspaceUser {
            metadata: ObjectMeta {
                name: Some(name.clone()),
                namespace: Some(namespace.clone()),
                labels: Some(BTreeMap::from([
                    ("platform.yurikrupnik.com/google-id".to_string(), user.id.clone()),
                    ("platform.yurikrupnik.com/domain".to_string(), config.spec.domain.clone()),
                    ("platform.yurikrupnik.com/managed-by".to_string(), "google-workspace-sync".to_string()),
                ])),
                ..Default::default()
            },
            spec: GoogleWorkspaceUserSpec {
                google_id: user.id,
                email: user.primary_email,
                full_name: user.name.full_name,
                given_name: user.name.given_name,
                family_name: user.name.family_name,
                org_unit_path: user.org_unit_path,
                suspended: user.suspended,
                is_admin: user.is_admin,
                is_delegated_admin: user.is_delegated_admin,
                groups: vec![], // Populated by group sync
                custom_attributes: user.custom_schemas,
            },
            status: Some(GoogleWorkspaceUserStatus {
                last_synced: Some(Utc::now().to_rfc3339()),
                ..Default::default()
            }),
        };

        api.patch(
            &name,
            &PatchParams::apply("google-workspace-sync"),
            &Patch::Apply(&gw_user),
        )
        .await
        .map_err(|e| OperatorError::GoogleWorkspace(format!("Failed to create user {}: {}", name, e)))?;

        count += 1;
    }

    Ok(count)
}

/// Sync groups from Google Workspace
async fn sync_groups(
    client: &kube::Client,
    config: &GoogleWorkspaceConfig,
    gw_client: &GoogleWorkspaceClient,
) -> Result<u32> {
    let groups = gw_client.list_groups().await?;
    let namespace = &config.spec.sync.target_namespace;
    let api: Api<GoogleWorkspaceGroup> = Api::namespaced(client.clone(), namespace);

    // Apply group filter if specified
    let filtered_groups: Vec<_> = if config.spec.sync.group_filter.is_empty() {
        groups
    } else {
        groups
            .into_iter()
            .filter(|g| config.spec.sync.group_filter.contains(&g.email))
            .collect()
    };

    let mut count = 0u32;

    for group in filtered_groups {
        let name = sanitize_name(&group.email);

        // Get group members
        let members = gw_client.get_group_members(&group.email).await.unwrap_or_default();

        let member_refs: Vec<GroupMemberRef> = members
            .iter()
            .filter(|m| m.member_type == "USER")
            .map(|m| GroupMemberRef {
                email: m.email.clone(),
                role: m.role.clone(),
                member_type: m.member_type.clone(),
            })
            .collect();

        let nested_groups: Vec<String> = members
            .iter()
            .filter(|m| m.member_type == "GROUP")
            .map(|m| m.email.clone())
            .collect();

        let gw_group = GoogleWorkspaceGroup {
            metadata: ObjectMeta {
                name: Some(name.clone()),
                namespace: Some(namespace.clone()),
                labels: Some(BTreeMap::from([
                    ("platform.yurikrupnik.com/google-id".to_string(), group.id.clone()),
                    ("platform.yurikrupnik.com/domain".to_string(), config.spec.domain.clone()),
                    ("platform.yurikrupnik.com/managed-by".to_string(), "google-workspace-sync".to_string()),
                ])),
                ..Default::default()
            },
            spec: GoogleWorkspaceGroupSpec {
                google_id: group.id,
                email: group.email,
                name: group.name,
                description: group.description,
                admin_created: group.admin_created,
                members: member_refs.clone(),
                nested_groups,
            },
            status: Some(GoogleWorkspaceGroupStatus {
                last_synced: Some(Utc::now().to_rfc3339()),
                member_count: member_refs.len() as u32,
                ..Default::default()
            }),
        };

        api.patch(
            &name,
            &PatchParams::apply("google-workspace-sync"),
            &Patch::Apply(&gw_group),
        )
        .await
        .map_err(|e| OperatorError::GoogleWorkspace(format!("Failed to create group {}: {}", name, e)))?;

        count += 1;
    }

    Ok(count)
}

/// Update user group memberships
async fn update_user_group_memberships(
    client: &kube::Client,
    config: &GoogleWorkspaceConfig,
    gw_client: &GoogleWorkspaceClient,
) -> Result<()> {
    let namespace = &config.spec.sync.target_namespace;
    let users_api: Api<GoogleWorkspaceUser> = Api::namespaced(client.clone(), namespace);

    // Get all synced users
    let users = users_api.list(&Default::default()).await?;

    for user in users.items {
        let email = &user.spec.email;

        // Get user's groups from GWS
        let user_groups = gw_client.get_user_groups(email).await.unwrap_or_default();
        let group_emails: Vec<String> = user_groups.iter().map(|g| g.email.clone()).collect();

        // Update user's groups field
        let patch = serde_json::json!({
            "spec": {
                "groups": group_emails
            }
        });

        users_api
            .patch(
                &user.name_any(),
                &PatchParams::apply("google-workspace-sync"),
                &Patch::Merge(&patch),
            )
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!(
                "Failed to update groups for user {}: {}", email, e
            )))?;
    }

    Ok(())
}

/// Update the phase of the GoogleWorkspaceConfig
async fn update_phase(
    api: &Api<GoogleWorkspaceConfig>,
    name: &str,
    phase: GoogleWorkspacePhase,
    message: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = GoogleWorkspaceConfigStatus {
        phase,
        message: Some(message.to_string()),
        conditions: vec![GoogleWorkspaceCondition {
            condition_type: "Reconciling".to_string(),
            status: "True".to_string(),
            reason: "Reconciling".to_string(),
            message: message.to_string(),
            last_transition_time: now,
        }],
        ..Default::default()
    };

    let patch = serde_json::json!({"status": status});

    api.patch_status(name, &PatchParams::apply("google-workspace-sync"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

/// Update the full status of the GoogleWorkspaceConfig
async fn update_status_full(
    api: &Api<GoogleWorkspaceConfig>,
    name: &str,
    users_synced: u32,
    groups_synced: u32,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = GoogleWorkspaceConfigStatus {
        ready: true,
        phase: GoogleWorkspacePhase::Ready,
        last_sync_time: Some(now.clone()),
        users_synced: Some(users_synced),
        groups_synced: Some(groups_synced),
        message: Some(format!("Synced {} users and {} groups", users_synced, groups_synced)),
        conditions: vec![GoogleWorkspaceCondition {
            condition_type: "Ready".to_string(),
            status: "True".to_string(),
            reason: "SyncComplete".to_string(),
            message: format!("Synced {} users and {} groups", users_synced, groups_synced),
            last_transition_time: now,
        }],
        ..Default::default()
    };

    let patch = serde_json::json!({"status": status});

    api.patch_status(name, &PatchParams::apply("google-workspace-sync"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

/// Sanitize a name for Kubernetes (lowercase, replace special chars)
fn sanitize_name(name: &str) -> String {
    name.to_lowercase()
        .replace('@', "-at-")
        .replace('.', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Parse a duration string like "5m" or "1h"
fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: u64 = num_str.parse().ok()?;

    match unit {
        "s" => Some(Duration::from_secs(num)),
        "m" => Some(Duration::from_secs(num * 60)),
        "h" => Some(Duration::from_secs(num * 3600)),
        "d" => Some(Duration::from_secs(num * 86400)),
        _ => None,
    }
}

/// Error policy for the controller
pub fn error_policy(
    config: Arc<GoogleWorkspaceConfig>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "Reconcile error for GoogleWorkspaceConfig {}: {}",
        config.name_any(),
        error
    );

    Action::requeue(Duration::from_secs(60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("user@example.com"), "user-at-example-com");
        assert_eq!(sanitize_name("Team-Admin@example.com"), "team-admin-at-example-com");
        assert_eq!(sanitize_name("user.name@company.org"), "user-name-at-company-org");
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("5m"), Some(Duration::from_secs(300)));
        assert_eq!(parse_duration("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration("1d"), Some(Duration::from_secs(86400)));
        assert_eq!(parse_duration("invalid"), None);
    }
}
