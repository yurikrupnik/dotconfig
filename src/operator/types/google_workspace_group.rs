//! GoogleWorkspaceGroup CRD - represents a synced Google Workspace group

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// GoogleWorkspaceGroup - represents a synced Google Workspace group
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "GoogleWorkspaceGroup",
    namespaced,
    status = "GoogleWorkspaceGroupStatus",
    shortname = "gwg",
    printcolumn = r#"{"name":"Email", "type":"string", "jsonPath":".spec.email"}"#,
    printcolumn = r#"{"name":"Name", "type":"string", "jsonPath":".spec.name"}"#,
    printcolumn = r#"{"name":"Members", "type":"integer", "jsonPath":".status.memberCount"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWorkspaceGroupSpec {
    /// Google Workspace group ID (immutable)
    pub google_id: String,

    /// Group email address
    pub email: String,

    /// Group name
    pub name: String,

    /// Group description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether this group was created by an admin
    #[serde(default)]
    pub admin_created: bool,

    /// Direct member emails (users only)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<GroupMemberRef>,

    /// Nested group emails
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nested_groups: Vec<String>,
}

/// Reference to a group member
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GroupMemberRef {
    /// Member email address
    pub email: String,

    /// Member role (OWNER, MANAGER, MEMBER)
    pub role: String,

    /// Member type (USER, GROUP)
    #[serde(rename = "type")]
    pub member_type: String,
}

/// Status of the GoogleWorkspaceGroup
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWorkspaceGroupStatus {
    /// Last sync time from Google Workspace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_synced: Option<String>,

    /// Total member count
    #[serde(default)]
    pub member_count: u32,

    /// Associated RBAC rules that reference this group
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub associated_rules: Vec<AssociatedRule>,

    /// Observed generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
}

/// Reference to an RBAC rule that uses this group
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssociatedRule {
    /// Name of the GoogleWorkspaceConfig
    pub config_name: String,

    /// Name of the rule
    pub rule_name: String,

    /// Resources this rule allows
    pub allowed_resources: Vec<String>,
}
