use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// CrossplaneResource CRD for managing Crossplane composite resources and claims
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "CrossplaneResource",
    namespaced,
    status = "CrossplaneResourceStatus",
    shortname = "xres",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Composition", "type":"string", "jsonPath":".spec.compositionRef.name"}"#,
    printcolumn = r#"{"name":"Ready", "type":"boolean", "jsonPath":".status.compositeResource.ready"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct CrossplaneResourceSpec {
    /// Reference to the Composition to use
    pub composition_ref: CompositionReference,

    /// Reference to an existing claim (if using claims API)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_ref: Option<ClaimReference>,

    /// Parameters to pass to the composition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,

    /// Where to write connection details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_connection_secret_to_ref: Option<ConnectionSecretRef>,

    /// Provider configuration reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_config_ref: Option<ProviderConfigRef>,

    /// Resource policies (deletion, update)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_policies: Option<ResourcePolicies>,

    /// Labels to apply to managed resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompositionReference {
    /// Name of the Composition
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimReference {
    /// API version of the claim
    pub api_version: String,

    /// Kind of the claim
    pub kind: String,

    /// Name of the claim
    pub name: String,

    /// Namespace of the claim
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSecretRef {
    /// Name of the secret to create
    pub name: String,

    /// Namespace for the secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigRef {
    /// Name of the ProviderConfig
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePolicies {
    /// Deletion policy: Delete or Orphan
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deletion_policy: Option<DeletionPolicy>,

    /// Update policy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_policy: Option<UpdatePolicy>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum DeletionPolicy {
    #[default]
    Delete,
    Orphan,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum UpdatePolicy {
    Automatic,
    Manual,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CrossplaneResourceStatus {
    /// Current phase of the resource
    pub phase: CrossplanePhase,

    /// Composite resource status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composite_resource: Option<CompositeResourceStatus>,

    /// Managed resources created by the composition
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub managed_resources: Vec<ManagedResourceStatus>,

    /// Connection details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_details: Option<ConnectionDetails>,

    /// Conditions for detailed status
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<CrossplaneCondition>,

    /// Observed generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,

    /// Last reconciliation time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reconcile_time: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum CrossplanePhase {
    #[default]
    Pending,
    Creating,
    Ready,
    Failed,
    Deleting,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompositeResourceStatus {
    /// Name of the XR (composite resource)
    pub name: String,

    /// Whether the XR is ready
    pub ready: bool,

    /// Synced status
    #[serde(default)]
    pub synced: bool,

    /// Composition revision used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composition_revision: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManagedResourceStatus {
    /// API version of the managed resource
    pub api_version: String,

    /// Kind of the managed resource
    pub kind: String,

    /// Name of the managed resource
    pub name: String,

    /// Whether the resource is ready
    pub ready: bool,

    /// Whether the resource is synced
    #[serde(default)]
    pub synced: bool,

    /// Provider-specific status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionDetails {
    /// Name of the secret containing connection details
    pub secret_name: String,

    /// Namespace of the secret
    pub secret_namespace: String,

    /// Available connection keys
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_keys: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CrossplaneCondition {
    /// Type of condition (Ready, Synced, etc.)
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
