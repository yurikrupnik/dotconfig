//! Authorization Validating Webhook
//!
//! Validates resource operations against RBAC policies defined in
//! GoogleWorkspaceConfig.

use crate::operator::types::{
    GoogleWorkspaceConfig, GoogleWorkspaceUser, RbacDefaultAction, RbacRule,
};
use crate::operator::Context;
use kube::api::ListParams;
use kube::core::admission::{AdmissionRequest, AdmissionResponse};
use kube::core::DynamicObject;
use kube::Api;
use tracing::{debug, info, warn};

/// Validate the resource operation against RBAC policies
pub async fn validate_authz(
    request: &AdmissionRequest<DynamicObject>,
    ctx: &Context,
) -> AdmissionResponse {
    // Only validate CREATE and UPDATE operations
    match request.operation {
        kube::core::admission::Operation::Create | kube::core::admission::Operation::Update => {}
        _ => return AdmissionResponse::from(request),
    }

    // Get the resource kind
    let resource_kind = &request.kind.kind;

    // Skip validation for GoogleWorkspace* resources (allow operators to manage them)
    if resource_kind.starts_with("GoogleWorkspace") {
        return AdmissionResponse::from(request);
    }

    // Extract user email
    let user_email = match extract_user_email(request) {
        Some(email) => email,
        None => {
            warn!("Could not extract user email, denying request");
            return AdmissionResponse::from(request)
                .deny("Could not identify user for authorization");
        }
    };

    info!(
        "Validating {:?} {} for user {}",
        request.operation,
        resource_kind,
        user_email
    );

    // Load RBAC policy from GoogleWorkspaceConfig
    let (default_action, rules) = match load_rbac_policy(ctx).await {
        Some((action, rules)) => (action, rules),
        None => {
            // No policy configured, allow by default
            debug!("No RBAC policy configured, allowing request");
            return AdmissionResponse::from(request);
        }
    };

    // Look up user's groups
    let user_groups = match lookup_user_groups(ctx, &user_email).await {
        Some(groups) => groups,
        None => {
            // User not found in GWS
            if default_action == RbacDefaultAction::Deny {
                return AdmissionResponse::from(request)
                    .deny(format!("User {} not found in Google Workspace", user_email));
            }
            return AdmissionResponse::from(request);
        }
    };

    // Get namespace from request
    let namespace = request.namespace.as_deref().unwrap_or("default");

    // Find matching rules
    let matching_rules: Vec<&RbacRule> = rules
        .iter()
        .filter(|rule| {
            // Check if user is in any of the rule's groups
            rule.groups.iter().any(|g| user_groups.contains(g))
        })
        .filter(|rule| {
            // Check if resource type is allowed
            rule.allowed_resources.contains(&resource_kind.to_string())
                || rule.allowed_resources.contains(&"*".to_string())
        })
        .filter(|rule| {
            // Check namespace restrictions
            rule.allowed_namespaces.is_empty()
                || rule.allowed_namespaces.contains(&namespace.to_string())
                || rule.allowed_namespaces.contains(&"*".to_string())
        })
        .collect();

    if matching_rules.is_empty() {
        // No matching rule
        if default_action == RbacDefaultAction::Deny {
            let required_groups = get_required_groups_for_resource(&rules, resource_kind);
            return AdmissionResponse::from(request).deny(format!(
                "User {} is not authorized to {:?} {} resources. Required groups: {:?}",
                user_email,
                request.operation,
                resource_kind,
                required_groups
            ));
        }
        return AdmissionResponse::from(request);
    }

    // Check resource quotas (if applicable for CREATE)
    if request.operation == kube::core::admission::Operation::Create {
        for rule in &matching_rules {
            if rule.max_resources_per_user > 0 {
                let current_count =
                    count_user_resources(ctx, &user_email, resource_kind, namespace).await;

                if current_count >= rule.max_resources_per_user {
                    return AdmissionResponse::from(request).deny(format!(
                        "User {} has reached the maximum of {} {} resources",
                        user_email, rule.max_resources_per_user, resource_kind
                    ));
                }
            }
        }
    }

    info!(
        "Authorized {:?} {} for user {} via rules: {:?}",
        request.operation,
        resource_kind,
        user_email,
        matching_rules.iter().map(|r| &r.name).collect::<Vec<_>>()
    );

    AdmissionResponse::from(request)
}

/// Extract user email from the admission request
fn extract_user_email(request: &AdmissionRequest<DynamicObject>) -> Option<String> {
    let user_info = &request.user_info;

    // Check if username is an email
    if let Some(username) = &user_info.username {
        if username.contains('@') {
            return Some(username.clone());
        }
    }

    // Check extra info for email claim
    if let Some(extra) = &user_info.extra {
        if let Some(emails) = extra.get("email") {
            if let Some(email) = emails.first() {
                return Some(email.clone());
            }
        }
    }

    user_info.username.clone()
}

/// Load RBAC policy from GoogleWorkspaceConfig
async fn load_rbac_policy(ctx: &Context) -> Option<(RbacDefaultAction, Vec<RbacRule>)> {
    let api: Api<GoogleWorkspaceConfig> = Api::all(ctx.client.clone());

    let configs = match api.list(&ListParams::default()).await {
        Ok(list) => list,
        Err(e) => {
            warn!("Failed to list GoogleWorkspaceConfig: {}", e);
            return None;
        }
    };

    // Use the first config with an RBAC policy
    for config in configs.items {
        if let Some(rbac) = &config.spec.rbac_policy {
            return Some((rbac.default_action.clone(), rbac.rules.clone()));
        }
    }

    None
}

/// Look up user's groups from GoogleWorkspaceUser
async fn lookup_user_groups(ctx: &Context, email: &str) -> Option<Vec<String>> {
    let namespaces = vec!["platform-users", "default"];

    for ns in namespaces {
        let api: Api<GoogleWorkspaceUser> = Api::namespaced(ctx.client.clone(), ns);

        let users = match api.list(&ListParams::default()).await {
            Ok(list) => list,
            Err(_) => continue,
        };

        for user in users.items {
            if user.spec.email == email {
                return Some(user.spec.groups);
            }
        }
    }

    None
}

/// Get the groups required to create a specific resource type
fn get_required_groups_for_resource(rules: &[RbacRule], resource_kind: &str) -> Vec<String> {
    rules
        .iter()
        .filter(|r| {
            r.allowed_resources.contains(&resource_kind.to_string())
                || r.allowed_resources.contains(&"*".to_string())
        })
        .flat_map(|r| r.groups.clone())
        .collect()
}

/// Count resources owned by a user
async fn count_user_resources(
    _ctx: &Context,
    user_email: &str,
    resource_kind: &str,
    namespace: &str,
) -> u32 {
    // This is a simplified implementation
    // In production, you'd query the specific resource type
    use crate::operator::webhooks::ownership::OWNER_EMAIL_ANNOTATION;

    // For now, we'll use a label selector approach
    // This requires resources to have an owner label (not just annotation)
    let label_selector = format!(
        "{}={}",
        OWNER_EMAIL_ANNOTATION.replace('/', "_"),
        user_email.replace('@', "_at_")
    );

    debug!(
        "Counting {} resources for {} in {} (selector: {})",
        resource_kind, user_email, namespace, label_selector
    );

    // Return 0 for now - in production you'd implement proper counting
    // This would require dynamic API access to the specific resource type
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_required_groups() {
        let rules = vec![
            RbacRule {
                name: "developers".to_string(),
                groups: vec!["devs@example.com".to_string()],
                allowed_resources: vec!["Bucket".to_string()],
                max_resources_per_user: 10,
                allowed_namespaces: vec![],
                conditions: None,
            },
            RbacRule {
                name: "admins".to_string(),
                groups: vec!["admins@example.com".to_string()],
                allowed_resources: vec!["*".to_string()],
                max_resources_per_user: 0,
                allowed_namespaces: vec![],
                conditions: None,
            },
        ];

        let groups = get_required_groups_for_resource(&rules, "Bucket");
        assert!(groups.contains(&"devs@example.com".to_string()));
        assert!(groups.contains(&"admins@example.com".to_string()));
    }
}
