//! BedrockAccess CRD for provisioning AWS Bedrock access via Crossplane
//!
//! This CRD abstracts the complexity of setting up Bedrock access with:
//! - IAM roles for IRSA (IAM Roles for Service Accounts)
//! - Model access permissions (foundation models, custom models)
//! - Knowledge base access (optional)
//! - Agent access (optional)
//! - Guardrails configuration
//!
//! Security requirements enforced:
//! - IRSA-only authentication (no static credentials)
//! - Least privilege IAM policies
//! - Model-specific permissions

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// Re-use shared Crossplane types
pub use super::crossplane_resource::{ConnectionSecretRef, DeletionPolicy, ProviderConfigRef};

/// BedrockAccess CRD for provisioning AWS Bedrock access via Crossplane
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "BedrockAccess",
    namespaced,
    status = "BedrockAccessStatus",
    shortname = "bedrock",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Region", "type":"string", "jsonPath":".spec.region"}"#,
    printcolumn = r#"{"name":"Models", "type":"integer", "jsonPath":".status.modelCount"}"#,
    printcolumn = r#"{"name":"Ready", "type":"boolean", "jsonPath":".status.ready"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct BedrockAccessSpec {
    // ============ Core Configuration ============

    /// AWS region for Bedrock
    pub region: String,

    /// Environment tier affecting defaults
    #[serde(default)]
    pub environment: BedrockEnvironment,

    // ============ IAM Configuration (IRSA Only) ============

    /// IAM configuration using IRSA (IAM Roles for Service Accounts)
    /// Security: Static credentials are not allowed
    pub iam: BedrockIamConfig,

    // ============ Model Access ============

    /// Foundation models to enable access to
    #[serde(default)]
    pub foundation_models: FoundationModelAccess,

    /// Custom model access (fine-tuned models)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_models: Vec<CustomModelAccess>,

    /// Provisioned throughput configurations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provisioned_throughput: Vec<ProvisionedThroughputSpec>,

    // ============ Knowledge Bases ============

    /// Knowledge base configurations for RAG
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub knowledge_bases: Vec<KnowledgeBaseSpec>,

    // ============ Agents ============

    /// Bedrock Agent configurations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<BedrockAgentSpec>,

    // ============ Guardrails ============

    /// Guardrails configuration for content filtering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardrails: Option<GuardrailsSpec>,

    // ============ Logging & Monitoring ============

    /// Model invocation logging configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<BedrockLoggingConfig>,

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
pub enum BedrockEnvironment {
    #[default]
    Dev,
    Staging,
    Prod,
}

// ============ IAM Configuration ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BedrockIamConfig {
    /// Service account name for IRSA
    pub service_account_name: String,

    /// Service account namespace
    pub service_account_namespace: String,

    /// OIDC provider ARN (if not using cluster default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oidc_provider_arn: Option<String>,

    /// Additional IAM policy statements
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_policy_statements: Vec<serde_json::Value>,
}

// ============ Foundation Model Access ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FoundationModelAccess {
    /// Allow access to all foundation models in the region
    #[serde(default)]
    pub allow_all: bool,

    /// Specific model IDs to allow (e.g., "anthropic.claude-3-sonnet-20240229-v1:0")
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_ids: Vec<String>,

    /// Model providers to allow (e.g., "anthropic", "amazon", "meta", "cohere")
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<BedrockModelProvider>,

    /// Model capabilities to allow
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<ModelCapability>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BedrockModelProvider {
    Anthropic,
    Amazon,
    Meta,
    Cohere,
    AI21,
    Mistral,
    Stability,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ModelCapability {
    TextGeneration,
    Chat,
    Embedding,
    ImageGeneration,
    ImageToText,
}

// ============ Custom Model Access ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CustomModelAccess {
    /// Custom model ARN or ID
    pub model_id: String,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ============ Provisioned Throughput ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionedThroughputSpec {
    /// Name for the provisioned throughput
    pub name: String,

    /// Model ARN to provision throughput for
    pub model_arn: String,

    /// Number of model units
    pub model_units: i32,

    /// Commitment duration
    #[serde(default)]
    pub commitment_duration: CommitmentDuration,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CommitmentDuration {
    /// No commitment (on-demand)
    #[default]
    None,
    /// One month commitment
    OneMonth,
    /// Six month commitment
    SixMonths,
}

// ============ Knowledge Base ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBaseSpec {
    /// Knowledge base name
    pub name: String,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Embedding model ID
    #[serde(default = "default_embedding_model")]
    pub embedding_model_id: String,

    /// Vector store configuration
    pub vector_store: VectorStoreConfig,

    /// Data source configurations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_sources: Vec<DataSourceSpec>,
}

fn default_embedding_model() -> String {
    "amazon.titan-embed-text-v1".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum VectorStoreConfig {
    /// OpenSearch Serverless
    OpenSearchServerless(OpenSearchServerlessConfig),
    /// Amazon Aurora PostgreSQL with pgvector
    AuroraPostgres(AuroraPostgresConfig),
    /// Pinecone
    Pinecone(PineconeConfig),
    /// Redis Enterprise
    Redis(RedisVectorConfig),
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OpenSearchServerlessConfig {
    /// Collection ARN (existing) or name to create
    pub collection: String,
    /// Create the collection if it doesn't exist
    #[serde(default)]
    pub create_collection: bool,
    /// Vector index name
    #[serde(default = "default_vector_index")]
    pub vector_index_name: String,
}

fn default_vector_index() -> String {
    "bedrock-knowledge-base-default-index".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuroraPostgresConfig {
    /// Database cluster ARN
    pub cluster_arn: String,
    /// Database name
    pub database_name: String,
    /// Table name for vectors
    pub table_name: String,
    /// Secret ARN for database credentials
    pub credentials_secret_arn: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PineconeConfig {
    /// Pinecone connection secret ARN
    pub connection_secret_arn: String,
    /// Index name
    pub index_name: String,
    /// Namespace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RedisVectorConfig {
    /// Redis endpoint
    pub endpoint: String,
    /// Credentials secret ARN
    pub credentials_secret_arn: String,
    /// Vector index name
    pub vector_index_name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DataSourceSpec {
    /// Data source name
    pub name: String,

    /// Data source type
    pub source: DataSourceType,

    /// Chunking strategy
    #[serde(default)]
    pub chunking_strategy: ChunkingStrategy,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DataSourceType {
    /// S3 bucket
    S3 {
        bucket_arn: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        prefix: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        inclusion_prefixes: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        exclusion_prefixes: Vec<String>,
    },
    /// Web crawler
    Web {
        seed_urls: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        scope: Option<WebCrawlerScope>,
    },
    /// Confluence
    Confluence {
        site_url: String,
        credentials_secret_arn: String,
    },
    /// SharePoint
    SharePoint {
        site_url: String,
        credentials_secret_arn: String,
    },
    /// Salesforce
    Salesforce {
        credentials_secret_arn: String,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum WebCrawlerScope {
    #[default]
    HostOnly,
    Subdomains,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChunkingStrategy {
    #[serde(default)]
    pub strategy_type: ChunkingType,
    /// Max tokens per chunk (for fixed size)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    /// Overlap percentage (for fixed size)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlap_percentage: Option<i32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ChunkingType {
    #[default]
    Default,
    FixedSize,
    None,
    Semantic,
    Hierarchical,
}

// ============ Bedrock Agent ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BedrockAgentSpec {
    /// Agent name
    pub name: String,

    /// Agent description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Foundation model for the agent
    pub foundation_model: String,

    /// Agent instructions
    pub instruction: String,

    /// Idle session timeout in seconds
    #[serde(default = "default_idle_timeout")]
    pub idle_session_ttl_in_seconds: i32,

    /// Action groups
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_groups: Vec<AgentActionGroup>,

    /// Knowledge bases to associate
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub knowledge_base_associations: Vec<String>,

    /// Guardrail to apply
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardrail_id: Option<String>,
}

fn default_idle_timeout() -> i32 {
    600
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentActionGroup {
    /// Action group name
    pub name: String,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Action group executor
    pub executor: ActionGroupExecutor,

    /// API schema (OpenAPI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_schema: Option<ApiSchema>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ActionGroupExecutor {
    /// Lambda function
    Lambda { lambda_arn: String },
    /// Return control to user
    ReturnControl,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ApiSchema {
    /// Inline schema
    Inline { schema: String },
    /// S3 location
    S3 { bucket: String, key: String },
}

// ============ Guardrails ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailsSpec {
    /// Guardrail name
    pub name: String,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Blocked input messaging
    #[serde(default = "default_blocked_message")]
    pub blocked_input_messaging: String,

    /// Blocked output messaging
    #[serde(default = "default_blocked_message")]
    pub blocked_outputs_messaging: String,

    /// Content filters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_filters: Option<ContentFiltersConfig>,

    /// Topic filters (denied topics)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub denied_topics: Vec<DeniedTopic>,

    /// Word filters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_filters: Option<WordFiltersConfig>,

    /// Sensitive information filters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitive_info_filters: Option<SensitiveInfoFiltersConfig>,

    /// Contextual grounding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contextual_grounding: Option<ContextualGroundingConfig>,
}

fn default_blocked_message() -> String {
    "Sorry, I cannot process this request.".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContentFiltersConfig {
    /// Hate content filter strength
    #[serde(default)]
    pub hate: FilterStrength,
    /// Insults filter strength
    #[serde(default)]
    pub insults: FilterStrength,
    /// Sexual content filter strength
    #[serde(default)]
    pub sexual: FilterStrength,
    /// Violence filter strength
    #[serde(default)]
    pub violence: FilterStrength,
    /// Misconduct filter strength
    #[serde(default)]
    pub misconduct: FilterStrength,
    /// Prompt attack filter strength
    #[serde(default)]
    pub prompt_attack: FilterStrength,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterStrength {
    None,
    #[default]
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeniedTopic {
    /// Topic name
    pub name: String,
    /// Topic definition
    pub definition: String,
    /// Example inputs that should be denied
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WordFiltersConfig {
    /// Profanity filter enabled
    #[serde(default = "default_true")]
    pub profanity_filter: bool,
    /// Custom words to block
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_words: Vec<String>,
    /// Managed word lists
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub managed_word_lists: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveInfoFiltersConfig {
    /// PII entities to filter
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pii_entities: Vec<PiiEntityConfig>,
    /// Regex patterns to filter
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regex_patterns: Vec<RegexPatternConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PiiEntityConfig {
    /// PII type (e.g., "EMAIL", "PHONE", "SSN", "CREDIT_CARD")
    pub pii_type: String,
    /// Action: BLOCK or ANONYMIZE
    #[serde(default)]
    pub action: PiiAction,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PiiAction {
    #[default]
    Block,
    Anonymize,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegexPatternConfig {
    /// Pattern name
    pub name: String,
    /// Regex pattern
    pub pattern: String,
    /// Action
    #[serde(default)]
    pub action: PiiAction,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextualGroundingConfig {
    /// Grounding threshold (0.0 - 1.0)
    #[serde(default = "default_grounding_threshold")]
    pub grounding_threshold: f64,
    /// Relevance threshold (0.0 - 1.0)
    #[serde(default = "default_relevance_threshold")]
    pub relevance_threshold: f64,
}

fn default_grounding_threshold() -> f64 {
    0.7
}

fn default_relevance_threshold() -> f64 {
    0.7
}

// ============ Logging Configuration ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BedrockLoggingConfig {
    /// Enable model invocation logging
    #[serde(default)]
    pub enabled: bool,

    /// CloudWatch log group ARN
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloudwatch_log_group_arn: Option<String>,

    /// S3 bucket for logs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s3_config: Option<S3LoggingConfig>,

    /// Log embeddings
    #[serde(default)]
    pub embedding_data_delivery_enabled: bool,

    /// Log image data
    #[serde(default)]
    pub image_data_delivery_enabled: bool,

    /// Log text data
    #[serde(default = "default_true")]
    pub text_data_delivery_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct S3LoggingConfig {
    /// S3 bucket ARN
    pub bucket_arn: String,
    /// Key prefix
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_prefix: Option<String>,
}

// ============ Status ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BedrockAccessStatus {
    /// Current phase
    pub phase: BedrockAccessPhase,

    /// Whether access is ready
    #[serde(default)]
    pub ready: bool,

    /// IAM role ARN
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iam_role_arn: Option<String>,

    /// Number of models accessible
    #[serde(default)]
    pub model_count: i32,

    /// Accessible model IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accessible_models: Vec<String>,

    /// Knowledge base IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub knowledge_base_ids: Vec<KnowledgeBaseStatus>,

    /// Agent IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agent_ids: Vec<AgentStatus>,

    /// Guardrail ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardrail_id: Option<String>,

    /// Managed Crossplane resources
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub managed_resources: Vec<ManagedBedrockResource>,

    /// Conditions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<BedrockAccessCondition>,

    /// Last reconcile time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reconcile_time: Option<String>,

    /// Message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum BedrockAccessPhase {
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
pub struct KnowledgeBaseStatus {
    pub name: String,
    pub knowledge_base_id: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub name: String,
    pub agent_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_alias_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManagedBedrockResource {
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
pub struct BedrockAccessCondition {
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
