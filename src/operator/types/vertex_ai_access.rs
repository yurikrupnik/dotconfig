//! VertexAIAccess CRD for provisioning Google Vertex AI access via Crossplane
//!
//! This CRD abstracts the complexity of setting up Vertex AI access with:
//! - GCP Service Accounts with Workload Identity
//! - Model access permissions (Gemini, PaLM, custom models)
//! - Vector Search indexes
//! - Feature Store access
//! - Model endpoints
//!
//! Security requirements enforced:
//! - Workload Identity only (no service account keys)
//! - Least privilege IAM bindings
//! - Project-scoped permissions

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// Re-use shared Crossplane types
pub use super::crossplane_resource::{ConnectionSecretRef, DeletionPolicy, ProviderConfigRef};

/// VertexAIAccess CRD for provisioning Google Vertex AI access via Crossplane
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "VertexAIAccess",
    namespaced,
    status = "VertexAIAccessStatus",
    shortname = "vertexai",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Project", "type":"string", "jsonPath":".spec.projectId"}"#,
    printcolumn = r#"{"name":"Region", "type":"string", "jsonPath":".spec.region"}"#,
    printcolumn = r#"{"name":"Ready", "type":"boolean", "jsonPath":".status.ready"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct VertexAIAccessSpec {
    // ============ Core Configuration ============

    /// GCP Project ID
    pub project_id: String,

    /// GCP region for Vertex AI
    pub region: String,

    /// Environment tier affecting defaults
    #[serde(default)]
    pub environment: VertexAIEnvironment,

    // ============ Workload Identity Configuration ============

    /// Workload Identity configuration (GKE Service Account binding)
    /// Security: Service account keys are not allowed
    pub workload_identity: WorkloadIdentityConfig,

    // ============ Model Access ============

    /// Generative AI models (Gemini, PaLM)
    #[serde(default)]
    pub generative_ai: GenerativeAIAccess,

    /// Custom trained models
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_models: Vec<CustomVertexModel>,

    /// Model endpoints to create/use
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub endpoints: Vec<EndpointSpec>,

    // ============ Vector Search ============

    /// Vector Search indexes for RAG
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vector_search_indexes: Vec<VectorSearchIndexSpec>,

    // ============ Feature Store ============

    /// Feature Store configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_store: Option<FeatureStoreSpec>,

    // ============ Pipelines ============

    /// Vertex AI Pipelines access
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipelines: Option<PipelinesConfig>,

    // ============ Experiments & MLOps ============

    /// Vertex AI Experiments access
    #[serde(default)]
    pub experiments_enabled: bool,

    /// Model Registry access
    #[serde(default)]
    pub model_registry_enabled: bool,

    /// Metadata Store access
    #[serde(default)]
    pub metadata_store_enabled: bool,

    // ============ Notebooks ============

    /// Managed notebooks/Workbench instances
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workbench_instances: Vec<WorkbenchInstanceSpec>,

    // ============ Logging & Monitoring ============

    /// Logging configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<VertexLoggingConfig>,

    // ============ Crossplane Configuration ============

    /// Crossplane ProviderConfig reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_config_ref: Option<ProviderConfigRef>,

    /// Secret to write connection details to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_connection_secret_to_ref: Option<ConnectionSecretRef>,

    /// Deletion policy
    #[serde(default)]
    pub deletion_policy: DeletionPolicy,

    /// Labels to apply to all managed resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,
}

// ============ Environment ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VertexAIEnvironment {
    #[default]
    Dev,
    Staging,
    Prod,
}

// ============ Workload Identity Configuration ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadIdentityConfig {
    /// Kubernetes service account name
    pub kubernetes_service_account: String,

    /// Kubernetes service account namespace
    pub kubernetes_namespace: String,

    /// GCP service account name (will be created)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gcp_service_account_name: Option<String>,

    /// Use existing GCP service account
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_gcp_service_account: Option<String>,

    /// Additional IAM roles to grant
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_roles: Vec<String>,
}

// ============ Generative AI Access ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenerativeAIAccess {
    /// Enable access to Gemini models
    #[serde(default = "default_true")]
    pub gemini_enabled: bool,

    /// Specific Gemini model versions to allow
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gemini_models: Vec<GeminiModel>,

    /// Enable PaLM 2 models (legacy)
    #[serde(default)]
    pub palm_enabled: bool,

    /// Enable Imagen (image generation)
    #[serde(default)]
    pub imagen_enabled: bool,

    /// Enable Codey (code generation)
    #[serde(default)]
    pub codey_enabled: bool,

    /// Enable text embeddings
    #[serde(default = "default_true")]
    pub embeddings_enabled: bool,

    /// Model tuning permissions
    #[serde(default)]
    pub tuning_enabled: bool,

    /// Grounding with Google Search
    #[serde(default)]
    pub grounding_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum GeminiModel {
    /// Gemini 1.5 Pro
    Gemini15Pro,
    /// Gemini 1.5 Flash
    Gemini15Flash,
    /// Gemini 1.0 Pro
    Gemini10Pro,
    /// Gemini 1.0 Pro Vision
    Gemini10ProVision,
    /// Gemini 2.0 Flash (experimental)
    Gemini20FlashExp,
}

// ============ Custom Models ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CustomVertexModel {
    /// Model resource name or ID
    pub model_id: String,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether to allow deployment
    #[serde(default)]
    pub allow_deployment: bool,
}

// ============ Endpoints ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EndpointSpec {
    /// Endpoint display name
    pub display_name: String,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Network for private endpoints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,

    /// Deployed model configurations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deployed_models: Vec<DeployedModelSpec>,

    /// Traffic split (model ID -> percentage)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub traffic_split: BTreeMap<String, i32>,

    /// Enable request/response logging
    #[serde(default)]
    pub enable_logging: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeployedModelSpec {
    /// Model ID to deploy
    pub model_id: String,

    /// Display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Machine type
    #[serde(default = "default_machine_type")]
    pub machine_type: String,

    /// Minimum replica count
    #[serde(default = "default_min_replicas")]
    pub min_replica_count: i32,

    /// Maximum replica count
    #[serde(default = "default_max_replicas")]
    pub max_replica_count: i32,

    /// Accelerator type (GPU)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accelerator_type: Option<AcceleratorType>,

    /// Accelerator count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accelerator_count: Option<i32>,
}

fn default_machine_type() -> String {
    "n1-standard-4".to_string()
}

fn default_min_replicas() -> i32 {
    1
}

fn default_max_replicas() -> i32 {
    3
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AcceleratorType {
    NvidiaTeslaK80,
    NvidiaTeslaP100,
    NvidiaTeslaP4,
    NvidiaTeslaT4,
    NvidiaTeslaV100,
    NvidiaTeslaA100,
    NvidiaA10080Gb,
    NvidiaL4,
    NvidiaH100,
    TpuV2,
    TpuV3,
    TpuV4Pod,
    TpuV5LitePod,
}

// ============ Vector Search ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VectorSearchIndexSpec {
    /// Index display name
    pub display_name: String,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Dimensions of the vectors
    pub dimensions: i32,

    /// Approximate neighbors count
    #[serde(default = "default_neighbors_count")]
    pub approximate_neighbors_count: i32,

    /// Distance measure type
    #[serde(default)]
    pub distance_measure_type: DistanceMeasureType,

    /// Shard size
    #[serde(default)]
    pub shard_size: ShardSize,

    /// Algorithm config
    #[serde(default)]
    pub algorithm_config: AlgorithmConfig,

    /// Index endpoint configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_endpoint: Option<IndexEndpointSpec>,
}

fn default_neighbors_count() -> i32 {
    150
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DistanceMeasureType {
    #[default]
    DotProductDistance,
    SquaredL2Distance,
    CosineDistance,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ShardSize {
    ShardSizeSmall,
    #[default]
    ShardSizeMedium,
    ShardSizeLarge,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlgorithmConfig {
    #[serde(default)]
    pub tree_ah_config: Option<TreeAhConfig>,
    #[serde(default)]
    pub brute_force_config: Option<BruteForceConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TreeAhConfig {
    /// Leaf node embedding count
    #[serde(default = "default_leaf_node_count")]
    pub leaf_node_embedding_count: i32,
    /// Leaf nodes to search percent
    #[serde(default = "default_leaf_search_percent")]
    pub leaf_nodes_to_search_percent: i32,
}

fn default_leaf_node_count() -> i32 {
    1000
}

fn default_leaf_search_percent() -> i32 {
    10
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct BruteForceConfig {}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IndexEndpointSpec {
    /// Endpoint display name
    pub display_name: String,

    /// Public endpoint enabled
    #[serde(default)]
    pub public_endpoint_enabled: bool,

    /// Private service connect config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_service_connect_config: Option<PrivateServiceConnectConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PrivateServiceConnectConfig {
    /// Enable private service connect
    pub enabled: bool,
    /// Project allowlist
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_allowlist: Vec<String>,
}

// ============ Feature Store ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeatureStoreSpec {
    /// Feature Store name
    pub name: String,

    /// Online serving configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_serving_config: Option<OnlineServingConfig>,

    /// Entity types to create
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_types: Vec<EntityTypeSpec>,

    /// Feature views
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feature_views: Vec<FeatureViewSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnlineServingConfig {
    /// Fixed node count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_node_count: Option<i32>,
    /// Scaling config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaling: Option<OnlineServingScaling>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnlineServingScaling {
    pub min_node_count: i32,
    pub max_node_count: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntityTypeSpec {
    /// Entity type ID
    pub entity_type_id: String,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Features
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<FeatureSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSpec {
    /// Feature ID
    pub feature_id: String,
    /// Value type
    pub value_type: FeatureValueType,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FeatureValueType {
    Bool,
    BoolArray,
    Double,
    DoubleArray,
    Int64,
    Int64Array,
    String,
    StringArray,
    Bytes,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeatureViewSpec {
    /// Feature view ID
    pub feature_view_id: String,
    /// BigQuery source
    #[serde(skip_serializing_if = "Option::is_none")]
    pub big_query_source: Option<BigQuerySource>,
    /// Sync config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_config: Option<SyncConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BigQuerySource {
    /// BigQuery URI
    pub uri: String,
    /// Entity ID columns
    pub entity_id_columns: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SyncConfig {
    /// Cron schedule
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
}

// ============ Pipelines ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelinesConfig {
    /// Enable pipeline execution
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// GCS bucket for pipeline artifacts
    pub artifact_bucket: String,

    /// Service account for pipeline execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_service_account: Option<String>,

    /// KMS key for encryption
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kms_key_name: Option<String>,
}

// ============ Workbench ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchInstanceSpec {
    /// Instance name
    pub name: String,

    /// Machine type
    #[serde(default = "default_workbench_machine")]
    pub machine_type: String,

    /// Accelerator config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accelerator_config: Option<WorkbenchAccelerator>,

    /// Boot disk type
    #[serde(default = "default_disk_type")]
    pub boot_disk_type: String,

    /// Boot disk size in GB
    #[serde(default = "default_disk_size")]
    pub boot_disk_size_gb: i32,

    /// Data disk type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_disk_type: Option<String>,

    /// Data disk size in GB
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_disk_size_gb: Option<i32>,

    /// Network
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,

    /// Subnet
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subnet: Option<String>,

    /// Disable public IP
    #[serde(default)]
    pub no_public_ip: bool,

    /// Idle shutdown
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_shutdown_config: Option<IdleShutdownConfig>,
}

fn default_workbench_machine() -> String {
    "n1-standard-4".to_string()
}

fn default_disk_type() -> String {
    "pd-ssd".to_string()
}

fn default_disk_size() -> i32 {
    100
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchAccelerator {
    /// Accelerator type
    pub accelerator_type: AcceleratorType,
    /// Core count
    pub core_count: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdleShutdownConfig {
    /// Idle timeout in minutes
    pub idle_timeout_minutes: i32,
    /// Enable idle shutdown
    #[serde(default = "default_true")]
    pub idle_shutdown: bool,
}

// ============ Logging ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VertexLoggingConfig {
    /// Enable prediction request/response logging
    #[serde(default)]
    pub prediction_logging_enabled: bool,

    /// BigQuery table for logs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bigquery_destination: Option<String>,

    /// Sampling rate (0.0 - 1.0)
    #[serde(default = "default_sampling_rate")]
    pub sampling_rate: f64,
}

fn default_sampling_rate() -> f64 {
    1.0
}

// ============ Status ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VertexAIAccessStatus {
    /// Current phase
    pub phase: VertexAIAccessPhase,

    /// Whether access is ready
    #[serde(default)]
    pub ready: bool,

    /// GCP Service Account email
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account_email: Option<String>,

    /// Workload Identity binding status
    #[serde(default)]
    pub workload_identity_bound: bool,

    /// Endpoint IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub endpoint_ids: Vec<EndpointStatus>,

    /// Vector Search index IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vector_search_index_ids: Vec<VectorSearchStatus>,

    /// Feature Store ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_store_id: Option<String>,

    /// Managed Crossplane resources
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub managed_resources: Vec<ManagedVertexResource>,

    /// Conditions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<VertexAIAccessCondition>,

    /// Last reconcile time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reconcile_time: Option<String>,

    /// Message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum VertexAIAccessPhase {
    #[default]
    Pending,
    Creating,
    Ready,
    Failed,
    Blocked,
    Deleting,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EndpointStatus {
    pub display_name: String,
    pub endpoint_id: String,
    pub resource_name: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VectorSearchStatus {
    pub display_name: String,
    pub index_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_id: Option<String>,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManagedVertexResource {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub ready: bool,
    #[serde(default)]
    pub synced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VertexAIAccessCondition {
    #[serde(rename = "type")]
    pub condition_type: String,
    pub status: String,
    pub reason: String,
    pub message: String,
    pub last_transition_time: String,
}

// ============ Helper Functions ============

fn default_true() -> bool {
    true
}
