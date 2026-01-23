use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// GitOpsApp CRD for managing FluxCD GitRepository and Kustomization resources
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "GitOpsApp",
    namespaced,
    status = "GitOpsAppStatus",
    shortname = "gapp",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Revision", "type":"string", "jsonPath":".status.lastSyncCommit"}"#,
    printcolumn = r#"{"name":"Suspended", "type":"boolean", "jsonPath":".spec.suspend"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct GitOpsAppSpec {
    /// Git repository configuration
    pub git_repository: GitRepositorySpec,

    /// Path in the repository to the manifests
    pub path: String,

    /// Reconciliation interval (e.g., "5m", "1h")
    #[serde(default = "default_interval")]
    pub interval: String,

    /// Enable garbage collection of resources
    #[serde(default = "default_true")]
    pub prune: bool,

    /// Suspend reconciliation
    #[serde(default)]
    pub suspend: bool,

    /// Target namespace for resources (if different from GitOpsApp namespace)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_namespace: Option<String>,

    /// Source type: kustomization or helm
    #[serde(default)]
    pub source_type: GitOpsSourceType,

    /// Helm-specific configuration (when source_type is helm)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub helm: Option<GitOpsHelmSpec>,

    /// Health checks to perform
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_checks: Option<Vec<HealthCheck>>,

    /// Depends on other GitOpsApps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<DependsOn>>,
}

fn default_interval() -> String {
    "5m".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GitOpsSourceType {
    #[default]
    Kustomization,
    Helm,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GitRepositorySpec {
    /// Repository URL (HTTPS or SSH)
    pub url: String,

    /// Branch to track
    #[serde(default = "default_branch")]
    pub branch: String,

    /// Tag to track (mutually exclusive with branch)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,

    /// Specific commit SHA
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,

    /// Secret reference for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<SecretReference>,

    /// Sync interval for the repository
    #[serde(default = "default_interval")]
    pub interval: String,
}

fn default_branch() -> String {
    "main".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretReference {
    /// Name of the secret
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GitOpsHelmSpec {
    /// Chart name in the repository
    pub chart: String,

    /// Values file path relative to repository root
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values_file: Option<String>,

    /// Inline values
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<serde_json::Value>,

    /// Release name override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    /// API version of the resource to check
    pub api_version: String,

    /// Kind of the resource
    pub kind: String,

    /// Name of the resource
    pub name: String,

    /// Namespace of the resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependsOn {
    /// Name of the GitOpsApp dependency
    pub name: String,

    /// Namespace of the dependency (defaults to same namespace)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GitOpsAppStatus {
    /// Current phase of the GitOps application
    pub phase: GitOpsPhase,

    /// Last successful sync time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_time: Option<String>,

    /// Last synced commit SHA
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_commit: Option<String>,

    /// Last applied revision
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_applied_revision: Option<String>,

    /// Resources managed by this GitOpsApp
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<ManagedResource>,

    /// FluxCD GitRepository name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_repository_name: Option<String>,

    /// FluxCD Kustomization/HelmRelease name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flux_resource_name: Option<String>,

    /// Conditions for detailed status
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<GitOpsCondition>,

    /// Observed generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum GitOpsPhase {
    #[default]
    Pending,
    Syncing,
    Synced,
    Failed,
    Suspended,
    Stalled,
    /// Blocked due to missing dependencies
    Blocked,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManagedResource {
    /// API version of the resource
    pub api_version: String,

    /// Kind of the resource
    pub kind: String,

    /// Name of the resource
    pub name: String,

    /// Namespace of the resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    /// Health status
    pub status: ResourceHealthStatus,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum ResourceHealthStatus {
    #[default]
    Unknown,
    Healthy,
    Unhealthy,
    Progressing,
    Suspended,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GitOpsCondition {
    /// Type of condition (Ready, Reconciling, Stalled, etc.)
    #[serde(rename = "type")]
    pub condition_type: String,

    /// Status: True, False, or Unknown
    pub status: String,

    /// Reason for the condition
    pub reason: String,

    /// Human-readable message
    pub message: String,

    /// Last transition time
    pub last_transition_time: String,
}
