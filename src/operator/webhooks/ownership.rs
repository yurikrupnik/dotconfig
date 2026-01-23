//! Ownership Mutating Webhook
//!
//! Injects ownership annotations on resource creation based on the
//! authenticated user from Google Workspace.

use crate::operator::types::GoogleWorkspaceUser;
use crate::operator::Context;
use chrono::Utc;
use kube::api::ListParams;
use kube::core::admission::{AdmissionRequest, AdmissionResponse};
use kube::core::DynamicObject;
use kube::Api;
use serde_json::json;
use tracing::{debug, info, warn};

/// Ownership annotation keys
pub const OWNER_EMAIL_ANNOTATION: &str = "platform.yurikrupnik.com/owner-email";
pub const OWNER_ID_ANNOTATION: &str = "platform.yurikrupnik.com/owner-id";
pub const OWNER_GROUPS_ANNOTATION: &str = "platform.yurikrupnik.com/owner-groups";
pub const CREATED_AT_ANNOTATION: &str = "platform.yurikrupnik.com/created-at";
pub const CREATED_BY_ANNOTATION: &str = "platform.yurikrupnik.com/created-by";

/// Mutate the resource to add ownership annotations
pub async fn mutate_ownership(
    request: &AdmissionRequest<DynamicObject>,
    ctx: &Context,
) -> AdmissionResponse {
    // Only mutate CREATE operations
    if request.operation != kube::core::admission::Operation::Create {
        return AdmissionResponse::from(request);
    }

    // Extract user email from the request
    let user_email = match extract_user_email(request) {
        Some(email) => email,
        None => {
            warn!("Could not extract user email from request");
            return AdmissionResponse::from(request);
        }
    };

    info!("Injecting ownership for user: {}", user_email);

    // Look up the GoogleWorkspaceUser
    let gws_user = match lookup_gws_user(ctx, &user_email).await {
        Some(user) => user,
        None => {
            debug!("No GoogleWorkspaceUser found for {}, using email only", user_email);
            // Still add basic ownership even without GWS user
            return create_ownership_patch(request, &user_email, None, vec![]);
        }
    };

    // Get user's groups
    let groups = gws_user.spec.groups.clone();
    let google_id = Some(gws_user.spec.google_id.clone());

    create_ownership_patch(request, &user_email, google_id, groups)
}

/// Extract user email from the admission request
fn extract_user_email(request: &AdmissionRequest<DynamicObject>) -> Option<String> {
    // Try to get email from user info
    let user_info = &request.user_info;

    // Check if username is an email (common with OIDC)
    if let Some(username) = &user_info.username {
        if username.contains('@') {
            return Some(username.clone());
        }
    }

    // Check extra info for email claim
    if let Some(extra) = &user_info.extra {
        // Google OIDC typically puts email in extra claims
        if let Some(emails) = extra.get("email") {
            if let Some(email) = emails.first() {
                return Some(email.clone());
            }
        }
    }

    // Fallback: try to extract from username format like "system:serviceaccount:namespace:name"
    // or just use the username if it looks like an identifier
    user_info.username.clone()
}

/// Look up a GoogleWorkspaceUser by email
async fn lookup_gws_user(ctx: &Context, email: &str) -> Option<GoogleWorkspaceUser> {
    // Try to find the user in the platform-users namespace (or configured namespace)
    let namespaces = vec!["platform-users", "default"];

    for ns in namespaces {
        let api: Api<GoogleWorkspaceUser> = Api::namespaced(ctx.client.clone(), ns);

        // List users and find by email
        let users = match api.list(&ListParams::default()).await {
            Ok(list) => list,
            Err(e) => {
                debug!("Failed to list users in {}: {}", ns, e);
                continue;
            }
        };

        for user in users.items {
            if user.spec.email == email {
                return Some(user);
            }
        }
    }

    None
}

/// Create the JSON patch for ownership annotations
fn create_ownership_patch(
    request: &AdmissionRequest<DynamicObject>,
    email: &str,
    google_id: Option<String>,
    groups: Vec<String>,
) -> AdmissionResponse {
    let now = Utc::now().to_rfc3339();

    // Build the annotations to add
    let mut annotations = serde_json::Map::new();
    annotations.insert(
        OWNER_EMAIL_ANNOTATION.to_string(),
        json!(email),
    );
    annotations.insert(
        CREATED_AT_ANNOTATION.to_string(),
        json!(now),
    );
    annotations.insert(
        CREATED_BY_ANNOTATION.to_string(),
        json!(format!("user:{}", email)),
    );

    if let Some(id) = google_id {
        annotations.insert(
            OWNER_ID_ANNOTATION.to_string(),
            json!(id),
        );
    }

    if !groups.is_empty() {
        annotations.insert(
            OWNER_GROUPS_ANNOTATION.to_string(),
            json!(groups.join(",")),
        );
    }

    // Create JSON patch
    // We need to handle the case where metadata.annotations might not exist
    let patches = if request.object.as_ref()
        .and_then(|o| o.metadata.annotations.as_ref())
        .is_some()
    {
        // Annotations exist, add to them
        annotations
            .into_iter()
            .map(|(k, v)| json!({
                "op": "add",
                "path": format!("/metadata/annotations/{}", k.replace('/', "~1")),
                "value": v
            }))
            .collect::<Vec<_>>()
    } else {
        // No annotations, create the whole object
        vec![json!({
            "op": "add",
            "path": "/metadata/annotations",
            "value": annotations
        })]
    };

    let patch = serde_json::Value::Array(patches);

    // Convert to json_patch::Patch
    let json_patch: json_patch::Patch = serde_json::from_value(patch)
        .expect("Failed to parse patch as json_patch::Patch");

    AdmissionResponse::from(request)
        .with_patch(json_patch)
        .expect("Failed to create patch")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_keys() {
        assert!(OWNER_EMAIL_ANNOTATION.starts_with("platform.yurikrupnik.com/"));
        assert!(OWNER_ID_ANNOTATION.starts_with("platform.yurikrupnik.com/"));
    }
}
