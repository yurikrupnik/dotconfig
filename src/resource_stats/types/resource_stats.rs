//! ResourceStats CRD representing collected cluster resource statistics

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::cost_config::Condition;

/// ResourceStats CRD representing collected cluster resource statistics
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "ResourceStats",
    namespaced,
    status = "ResourceStatsStatus",
    shortname = "rstats",
    printcolumn = r#"{"name":"Scope", "type":"string", "jsonPath":".spec.scope"}"#,
    printcolumn = r#"{"name":"Target", "type":"string", "jsonPath":".spec.targetRef.name"}"#,
    printcolumn = r#"{"name":"Cost/Hour", "type":"string", "jsonPath":".status.costSummary.totalPerHour"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct ResourceStatsSpec {
    /// Scope of metrics collection
    pub scope: StatsScope,

    /// Target reference (node, namespace, or deployment)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<TargetReference>,

    /// Label selector for pods/nodes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<BTreeMap<String, String>>,

    /// Collection interval
    #[serde(default = "default_interval")]
    pub interval: String,

    /// Retention period for historical data
    #[serde(default = "default_retention")]
    pub retention: String,

    /// Cost config reference to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_config_ref: Option<CostConfigReference>,

    /// Enable GPU metrics collection
    #[serde(default)]
    pub collect_gpu: bool,
}

fn default_interval() -> String {
    "1m".to_string()
}

fn default_retention() -> String {
    "24h".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StatsScope {
    Cluster,
    Node,
    Namespace,
    Deployment,
    Pod,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TargetReference {
    pub kind: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CostConfigReference {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

/// Status for ResourceStats CRD (the actual metrics data)
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourceStatsStatus {
    pub phase: StatsPhase,

    /// Current resource usage
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<ResourceSnapshot>,

    /// Cost summary
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_summary: Option<CostSummary>,

    /// Node-level breakdown (for cluster scope)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_stats: Vec<NodeResourceStats>,

    /// Pod-level breakdown
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pod_stats: Vec<PodResourceStats>,

    /// GPU stats
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpu_stats: Vec<GpuResourceStats>,

    /// Historical samples (for trending)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<HistoricalSample>,

    /// Last collection time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_collection_time: Option<String>,

    /// Conditions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum StatsPhase {
    #[default]
    Pending,
    Collecting,
    Ready,
    Stale,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSnapshot {
    pub timestamp: String,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu: Option<GpuMetrics>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CpuMetrics {
    /// Current usage in millicores
    pub usage_millicores: i64,
    /// Allocatable/capacity in millicores
    pub capacity_millicores: i64,
    /// Requests sum in millicores
    pub requests_millicores: i64,
    /// Limits sum in millicores
    pub limits_millicores: i64,
    /// Usage percentage
    pub usage_percent: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryMetrics {
    /// Current usage in bytes
    pub usage_bytes: i64,
    /// Allocatable/capacity in bytes
    pub capacity_bytes: i64,
    /// Requests sum in bytes
    pub requests_bytes: i64,
    /// Limits sum in bytes
    pub limits_bytes: i64,
    /// Usage percentage
    pub usage_percent: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GpuMetrics {
    /// Total GPUs available
    pub total_gpus: i32,
    /// GPUs in use
    pub used_gpus: i32,
    /// GPU memory used (bytes)
    pub memory_used_bytes: i64,
    /// GPU memory total (bytes)
    pub memory_total_bytes: i64,
    /// Average GPU utilization percent
    pub utilization_percent: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CostSummary {
    /// Currency
    pub currency: String,
    /// Total cost per hour
    pub total_per_hour: String,
    /// CPU cost per hour
    pub cpu_per_hour: String,
    /// Memory cost per hour
    pub memory_per_hour: String,
    /// GPU cost per hour
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_per_hour: Option<String>,
    /// Projected monthly cost (total_per_hour * 730)
    pub projected_monthly: String,
    /// Cost efficiency score (actual usage / requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub efficiency_score: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeResourceStats {
    pub node_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_type: Option<String>,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_hour: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PodResourceStats {
    pub pod_name: String,
    pub namespace: String,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_hour: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GpuResourceStats {
    pub node_name: String,
    pub vendor: String,
    pub model: String,
    pub gpu_index: i32,
    pub utilization_percent: f64,
    pub memory_used_bytes: i64,
    pub memory_total_bytes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature_celsius: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_watts: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_hour: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalSample {
    pub timestamp: String,
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_usage_percent: Option<f64>,
    pub cost_per_hour: String,
}
