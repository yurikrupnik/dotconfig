use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// PlatformApp CRD for installing applications via Helm and/or KCL manifests
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "PlatformApp",
    namespaced,
    status = "PlatformAppStatus",
    shortname = "papp",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Type", "type":"string", "jsonPath":".spec.installationType"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct PlatformAppSpec {
    /// Application name
    pub name: String,

    /// Target namespace for installation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    /// Installation type: "helm", "kcl", or "both"
    pub installation_type: InstallationType,

    /// Helm chart configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub helm: Option<HelmSpec>,

    /// KCL manifest configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kcl: Option<KclSpec>,

    /// Labels to apply to all created resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,

    /// Annotations to apply to all created resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InstallationType {
    Helm,
    Kcl,
    Cue,
    All,
}

impl Default for InstallationType {
    fn default() -> Self {
        Self::Cue
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HelmSpec {
    /// Helm chart name or path
    pub chart: String,

    /// Helm repository URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,

    /// Chart version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Values to pass to the chart
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<serde_json::Value>,

    /// Reference to a Secret containing values
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values_from: Option<Vec<ValuesReference>>,

    /// Wait for resources to be ready
    #[serde(default = "default_true")]
    pub wait: bool,

    /// Timeout for Helm operations in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u32,
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> u32 {
    300
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValuesReference {
    /// Kind of the values source (Secret or ConfigMap)
    pub kind: String,

    /// Name of the values source
    pub name: String,

    /// Key in the source containing values
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KclSpec {
    /// KCL source: OCI registry path (oci://), git URL, or local path
    pub source: String,

    /// Arguments to pass to KCL (-D key=value)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<BTreeMap<String, serde_json::Value>>,

    /// Pipeline steps following the function_registry.k pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_steps: Option<Vec<PipelineStep>>,

    /// Settings for KCL execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<KclSettings>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineStep {
    /// Function name from the registry
    pub function_name: String,

    /// Input data for the function
    pub input: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KclSettings {
    /// Disable None value output
    #[serde(default)]
    pub disable_none: bool,

    /// Sort keys in output
    #[serde(default = "default_true")]
    pub sort_keys: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlatformAppStatus {
    /// Current phase of the application
    pub phase: PlatformAppPhase,

    /// Human-readable message about current state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Helm release status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub helm_release: Option<HelmReleaseStatus>,

    /// KCL output status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kcl_output: Option<KclOutputStatus>,

    /// Last time the resource was reconciled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reconcile_time: Option<String>,

    /// Observed generation for status tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,

    /// Conditions for detailed status
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum PlatformAppPhase {
    #[default]
    Pending,
    Installing,
    Ready,
    Failed,
    Upgrading,
    Deleting,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HelmReleaseStatus {
    /// Current revision number
    pub revision: i32,

    /// Release status (deployed, failed, etc.)
    pub status: String,

    /// Last applied timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_applied: Option<String>,

    /// Chart version that was installed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart_version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KclOutputStatus {
    /// Number of resources created
    pub resources_created: i32,

    /// Last applied timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_applied: Option<String>,

    /// List of created resource references
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<ResourceReference>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourceReference {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// Type of condition
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
