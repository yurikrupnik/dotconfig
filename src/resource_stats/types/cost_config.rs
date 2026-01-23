//! CostConfig CRD for defining resource pricing

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// CostConfig CRD for defining resource pricing
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "CostConfig",
    namespaced,
    status = "CostConfigStatus",
    shortname = "ccfg",
    printcolumn = r#"{"name":"Source", "type":"string", "jsonPath":".spec.source"}"#,
    printcolumn = r#"{"name":"Provider", "type":"string", "jsonPath":".spec.cloudProvider"}"#,
    printcolumn = r#"{"name":"Active", "type":"boolean", "jsonPath":".status.active"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct CostConfigSpec {
    /// Pricing source: "static", "cloud", "hybrid"
    pub source: CostSource,

    /// Cloud provider for API-based pricing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_provider: Option<CloudProvider>,

    /// Static pricing configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_pricing: Option<StaticPricingSpec>,

    /// Cloud provider credentials reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials_ref: Option<SecretReference>,

    /// Region for pricing (affects cloud API rates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,

    /// Currency for cost display (USD, EUR, etc.)
    #[serde(default = "default_currency")]
    pub currency: String,

    /// Refresh interval for cloud pricing
    #[serde(default = "default_pricing_refresh")]
    pub refresh_interval: String,

    /// Node selectors - which nodes this config applies to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_selector: Option<BTreeMap<String, String>>,

    /// Priority (higher wins when multiple configs match)
    #[serde(default)]
    pub priority: i32,
}

fn default_currency() -> String {
    "USD".to_string()
}

fn default_pricing_refresh() -> String {
    "1h".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CostSource {
    Static,
    Cloud,
    Hybrid,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CloudProvider {
    Gcp,
    Aws,
    Azure,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaticPricingSpec {
    /// CPU cost per core per hour
    pub cpu_per_core_hour: String,

    /// Memory cost per GiB per hour
    pub memory_per_gib_hour: String,

    /// GPU pricing by type
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpu_pricing: Vec<GpuPricing>,

    /// Node type specific overrides
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_type_overrides: Vec<NodeTypePricing>,

    /// Storage cost per GiB per hour
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_per_gib_hour: Option<String>,

    /// Network egress cost per GiB
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_egress_per_gib: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GpuPricing {
    /// GPU vendor: nvidia, amd, intel
    pub vendor: GpuVendor,

    /// GPU model pattern (regex match)
    pub model_pattern: String,

    /// Cost per GPU per hour
    pub per_gpu_hour: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeTypePricing {
    /// Node label selector (e.g., node.kubernetes.io/instance-type)
    pub instance_type: String,

    /// Override CPU cost
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_per_core_hour: Option<String>,

    /// Override memory cost
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_per_gib_hour: Option<String>,

    /// Fixed cost per node per hour (spot vs on-demand)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_per_node_hour: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretReference {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub key: String,
}

/// Condition for CRD status
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    pub r#type: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub last_transition_time: String,
}

/// Status for CostConfig CRD
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CostConfigStatus {
    pub phase: CostConfigPhase,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum CostConfigPhase {
    #[default]
    Pending,
    Syncing,
    Ready,
    Failed,
}
