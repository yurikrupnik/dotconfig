//! GoogleWorkspaceConfig CRD - cluster-scoped configuration for Google Workspace integration

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// GoogleWorkspaceConfig - cluster-scoped configuration for Google Workspace integration
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "GoogleWorkspaceConfig",
    status = "GoogleWorkspaceConfigStatus",
    shortname = "gwc",
    printcolumn = r#"{"name":"Domain", "type":"string", "jsonPath":".spec.domain"}"#,
    printcolumn = r#"{"name":"Ready", "type":"boolean", "jsonPath":".status.ready"}"#,
    printcolumn = r#"{"name":"Users", "type":"integer", "jsonPath":".status.usersSynced"}"#,
    printcolumn = r#"{"name":"Groups", "type":"integer", "jsonPath":".status.groupsSynced"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWorkspaceConfigSpec {
    /// Google Workspace domain (e.g., "company.com")
    pub domain: String,

    /// Customer ID (found in Admin Console > Account > Account Settings)
    pub customer_id: String,

    /// Service account authentication configuration
    pub auth: GoogleWorkspaceAuthSpec,

    /// Sync configuration
    #[serde(default)]
    pub sync: SyncConfig,

    /// RBAC policy configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rbac_policy: Option<RbacPolicySpec>,
}

/// Authentication configuration for Google Workspace
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWorkspaceAuthSpec {
    /// Secret reference containing service account JSON key
    /// The service account must have domain-wide delegation enabled
    pub service_account_key_ref: GwsSecretKeySelector,

    /// Admin email to impersonate (required for domain-wide delegation)
    /// Must be a super admin in Google Workspace
    pub admin_email: String,

    /// OAuth scopes to request
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
}

fn default_scopes() -> Vec<String> {
    vec![
        "https://www.googleapis.com/auth/admin.directory.user.readonly".to_string(),
        "https://www.googleapis.com/auth/admin.directory.group.readonly".to_string(),
        "https://www.googleapis.com/auth/admin.directory.group.member.readonly".to_string(),
    ]
}

/// Reference to a key in a Kubernetes Secret for Google Workspace
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GwsSecretKeySelector {
    /// Name of the Secret
    pub name: String,

    /// Key within the Secret
    pub key: String,

    /// Namespace of the Secret (defaults to platform-system)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

/// Sync configuration for users and groups
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfig {
    /// Interval for syncing users/groups (default: 5m)
    #[serde(default = "default_sync_interval")]
    pub interval: String,

    /// Which groups to sync (if empty, sync all)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_filter: Vec<String>,

    /// Whether to sync users
    #[serde(default = "default_true")]
    pub sync_users: bool,

    /// Whether to sync groups
    #[serde(default = "default_true")]
    pub sync_groups: bool,

    /// Target namespace for synced resources
    #[serde(default = "default_namespace")]
    pub target_namespace: String,

    /// Whether to delete synced resources when user/group is removed from GWS
    #[serde(default)]
    pub prune_deleted: bool,
}

fn default_sync_interval() -> String {
    "5m".to_string()
}

fn default_true() -> bool {
    true
}

fn default_namespace() -> String {
    "platform-users".to_string()
}

/// RBAC policy for resource authorization
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RbacPolicySpec {
    /// Default behavior when no policy matches
    #[serde(default)]
    pub default_action: RbacDefaultAction,

    /// Rules for resource creation authorization
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<RbacRule>,
}

/// Default action when no RBAC rule matches
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RbacDefaultAction {
    #[default]
    Deny,
    Allow,
}

/// RBAC rule for authorizing resource operations
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RbacRule {
    /// Name of the rule
    pub name: String,

    /// Google Workspace groups that this rule applies to
    pub groups: Vec<String>,

    /// Resource types this rule allows (e.g., "Bucket", "PlatformApp")
    pub allowed_resources: Vec<String>,

    /// Maximum number of resources per user (0 = unlimited)
    #[serde(default)]
    pub max_resources_per_user: u32,

    /// Namespace restrictions (if empty, all namespaces allowed)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_namespaces: Vec<String>,

    /// Additional conditions (provider-specific parameters)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<BTreeMap<String, String>>,
}

/// Status of the GoogleWorkspaceConfig
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWorkspaceConfigStatus {
    /// Whether the config is ready and syncing
    #[serde(default)]
    pub ready: bool,

    /// Current phase
    #[serde(default)]
    pub phase: GoogleWorkspacePhase,

    /// Last successful sync time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_time: Option<String>,

    /// Number of users synced
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users_synced: Option<u32>,

    /// Number of groups synced
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups_synced: Option<u32>,

    /// Status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Observed generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,

    /// Conditions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<GoogleWorkspaceCondition>,
}

/// Phase of the GoogleWorkspaceConfig
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum GoogleWorkspacePhase {
    #[default]
    Pending,
    Initializing,
    Syncing,
    Ready,
    Failed,
}

/// Condition for GoogleWorkspaceConfig status
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GoogleWorkspaceCondition {
    #[serde(rename = "type")]
    pub condition_type: String,
    pub status: String,
    pub reason: String,
    pub message: String,
    pub last_transition_time: String,
}
