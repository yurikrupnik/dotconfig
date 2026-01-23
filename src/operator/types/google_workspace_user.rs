//! GoogleWorkspaceUser CRD - represents a synced Google Workspace user

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// GoogleWorkspaceUser - represents a synced Google Workspace user
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "GoogleWorkspaceUser",
    namespaced,
    status = "GoogleWorkspaceUserStatus",
    shortname = "gwu",
    printcolumn = r#"{"name":"Email", "type":"string", "jsonPath":".spec.email"}"#,
    printcolumn = r#"{"name":"Name", "type":"string", "jsonPath":".spec.fullName"}"#,
    printcolumn = r#"{"name":"Suspended", "type":"boolean", "jsonPath":".spec.suspended"}"#,
    printcolumn = r#"{"name":"Admin", "type":"boolean", "jsonPath":".spec.isAdmin"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWorkspaceUserSpec {
    /// Google Workspace user ID (immutable)
    pub google_id: String,

    /// Primary email address
    pub email: String,

    /// Full name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,

    /// Given (first) name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub given_name: Option<String>,

    /// Family (last) name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name: Option<String>,

    /// Organizational unit path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_unit_path: Option<String>,

    /// Whether the user is suspended
    #[serde(default)]
    pub suspended: bool,

    /// Whether the user is admin
    #[serde(default)]
    pub is_admin: bool,

    /// Whether the user is delegated admin
    #[serde(default)]
    pub is_delegated_admin: bool,

    /// Groups this user belongs to (group email addresses)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,

    /// Custom attributes from Google Workspace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_attributes: Option<serde_json::Value>,
}

/// Status of the GoogleWorkspaceUser
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWorkspaceUserStatus {
    /// Last sync time from Google Workspace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_synced: Option<String>,

    /// Number of resources owned by this user
    #[serde(default)]
    pub owned_resources_count: u32,

    /// Effective permissions (computed from group memberships)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effective_permissions: Vec<EffectivePermission>,

    /// Observed generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
}

/// Computed effective permission for a user
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EffectivePermission {
    /// Resource type this permission applies to
    pub resource_type: String,

    /// Allowed namespaces (empty = all)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_namespaces: Vec<String>,

    /// Maximum resources of this type
    pub max_resources: u32,

    /// Source group that granted this permission
    pub source_group: String,

    /// Source rule name
    pub source_rule: String,
}

impl GoogleWorkspaceUser {
    /// Check if the user has permission to create a resource
    pub fn can_create(&self, resource_type: &str, namespace: &str) -> bool {
        if let Some(status) = &self.status {
            for perm in &status.effective_permissions {
                if perm.resource_type == resource_type {
                    // Check namespace restriction
                    if perm.allowed_namespaces.is_empty()
                        || perm.allowed_namespaces.contains(&namespace.to_string())
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Get the resource limit for a specific type
    pub fn resource_limit(&self, resource_type: &str) -> Option<u32> {
        if let Some(status) = &self.status {
            for perm in &status.effective_permissions {
                if perm.resource_type == resource_type {
                    return Some(perm.max_resources);
                }
            }
        }
        None
    }
}
