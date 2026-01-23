use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// ExternalSecretConfig CRD for managing External Secrets Operator resources
/// Creates and manages ClusterSecretStore, ClusterExternalSecret, and ExternalSecret
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "ExternalSecretConfig",
    namespaced,
    status = "ExternalSecretConfigStatus",
    shortname = "esc",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Provider", "type":"string", "jsonPath":".spec.provider.type"}"#,
    printcolumn = r#"{"name":"Synced", "type":"boolean", "jsonPath":".status.synced"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSecretConfigSpec {
    /// Secret provider configuration (creates ClusterSecretStore)
    pub provider: SecretProviderSpec,

    /// Secrets to sync (creates ExternalSecret or ClusterExternalSecret)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<SecretSyncSpec>,

    /// Create as cluster-scoped resources (ClusterSecretStore + ClusterExternalSecret)
    #[serde(default = "default_true")]
    pub cluster_scoped: bool,

    /// Namespace selectors for ClusterExternalSecret propagation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace_selector: Option<NamespaceSelector>,

    /// Default refresh interval for all secrets
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: String,
}

fn default_true() -> bool {
    true
}

fn default_refresh_interval() -> String {
    "1h".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretProviderSpec {
    /// Provider type
    #[serde(rename = "type")]
    pub provider_type: SecretProviderType,

    /// Name for the SecretStore/ClusterSecretStore
    pub name: String,

    /// GCP Secret Manager configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gcp: Option<GcpProviderSpec>,

    /// AWS Secrets Manager configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aws: Option<AwsProviderSpec>,

    /// Azure Key Vault configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure: Option<AzureProviderSpec>,

    /// Vault configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vault: Option<VaultProviderSpec>,

    /// 1Password configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onepassword: Option<OnePasswordProviderSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SecretProviderType {
    Gcp,
    Aws,
    Azure,
    Vault,
    OnePassword,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GcpProviderSpec {
    /// GCP project ID
    pub project_id: String,

    /// Authentication configuration
    pub auth: GcpAuthSpec,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GcpAuthSpec {
    /// Secret reference for service account key
    pub secret_ref: SecretKeySelector,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AwsProviderSpec {
    /// AWS region
    pub region: String,

    /// Service type (SecretsManager or ParameterStore)
    #[serde(default = "default_aws_service")]
    pub service: AwsService,

    /// Authentication configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AwsAuthSpec>,
}

fn default_aws_service() -> AwsService {
    AwsService::SecretsManager
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum AwsService {
    #[default]
    SecretsManager,
    ParameterStore,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AwsAuthSpec {
    /// Secret reference for AWS credentials
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<AwsSecretRef>,

    /// Use IRSA (IAM Roles for Service Accounts)
    #[serde(default)]
    pub use_irsa: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AwsSecretRef {
    /// Secret containing accessKeyId
    pub access_key_id: SecretKeySelector,

    /// Secret containing secretAccessKey
    pub secret_access_key: SecretKeySelector,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AzureProviderSpec {
    /// Azure tenant ID
    pub tenant_id: String,

    /// Key Vault URL
    pub vault_url: String,

    /// Authentication configuration
    pub auth: AzureAuthSpec,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AzureAuthSpec {
    /// Secret reference for client credentials
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<AzureSecretRef>,

    /// Use Workload Identity
    #[serde(default)]
    pub use_workload_identity: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AzureSecretRef {
    /// Client ID
    pub client_id: SecretKeySelector,

    /// Client secret
    pub client_secret: SecretKeySelector,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultProviderSpec {
    /// Vault server address
    pub server: String,

    /// Vault path (e.g., "secret/data")
    pub path: String,

    /// Vault version (v1 or v2)
    #[serde(default = "default_vault_version")]
    pub version: String,

    /// Authentication configuration
    pub auth: VaultAuthSpec,
}

fn default_vault_version() -> String {
    "v2".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultAuthSpec {
    /// Token auth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_secret_ref: Option<SecretKeySelector>,

    /// Kubernetes auth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kubernetes: Option<VaultKubernetesAuth>,

    /// AppRole auth
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_role: Option<VaultAppRoleAuth>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultKubernetesAuth {
    /// Vault mount path for kubernetes auth
    pub mount_path: String,

    /// Vault role
    pub role: String,

    /// Service account reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account_ref: Option<ServiceAccountRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultAppRoleAuth {
    /// Vault mount path for approle auth
    pub path: String,

    /// Role ID secret reference
    pub role_id: SecretKeySelector,

    /// Secret ID secret reference
    pub secret_id: SecretKeySelector,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnePasswordProviderSpec {
    /// 1Password Connect server URL
    pub connect_host: String,

    /// Vault UUIDs to access
    pub vaults: BTreeMap<String, String>,

    /// Authentication configuration
    pub auth: OnePasswordAuthSpec,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnePasswordAuthSpec {
    /// Secret reference for connect token
    pub secret_ref: SecretKeySelector,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretKeySelector {
    /// Name of the secret
    pub name: String,

    /// Key in the secret
    pub key: String,

    /// Namespace of the secret (for cluster-scoped stores)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceAccountRef {
    /// Name of the service account
    pub name: String,

    /// Namespace of the service account
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretSyncSpec {
    /// Name for the ExternalSecret resource
    pub name: String,

    /// Target K8s secret name (defaults to ExternalSecret name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_secret_name: Option<String>,

    /// Target namespace (for namespace-scoped ExternalSecret)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_namespace: Option<String>,

    /// Refresh interval override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_interval: Option<String>,

    /// Individual secret data mappings
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<SecretDataSpec>,

    /// Extract all keys from a remote secret
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_from: Vec<SecretDataFromSpec>,

    /// Template for the target secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<SecretTemplateSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretDataSpec {
    /// Key in the target K8s secret
    pub secret_key: String,

    /// Remote reference
    pub remote_ref: RemoteRefSpec,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRefSpec {
    /// Key/path in the remote secret store
    pub key: String,

    /// Property within the secret (for JSON secrets)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,

    /// Version of the secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretDataFromSpec {
    /// Extract from a remote secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extract: Option<ExtractSpec>,

    /// Find secrets matching a pattern
    #[serde(skip_serializing_if = "Option::is_none")]
    pub find: Option<FindSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExtractSpec {
    /// Key/path in the remote secret store
    pub key: String,

    /// Version of the secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindSpec {
    /// Name pattern (regex)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<FindNameSpec>,

    /// Tags to match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FindNameSpec {
    /// Regex pattern for secret names
    pub regexp: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretTemplateSpec {
    /// Type of the target secret
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub secret_type: Option<String>,

    /// Annotations for the target secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SecretTemplateMetadata>,

    /// Template data with Go templating
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretTemplateMetadata {
    /// Labels for the target secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,

    /// Annotations for the target secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceSelector {
    /// Match labels
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_labels: Option<BTreeMap<String, String>>,

    /// Match expressions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_expressions: Option<Vec<LabelSelectorRequirement>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LabelSelectorRequirement {
    /// Label key
    pub key: String,

    /// Operator (In, NotIn, Exists, DoesNotExist)
    pub operator: String,

    /// Values for In/NotIn
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSecretConfigStatus {
    /// Current phase
    pub phase: ExternalSecretPhase,

    /// Whether secrets are synced
    #[serde(default)]
    pub synced: bool,

    /// SecretStore/ClusterSecretStore name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_store_name: Option<String>,

    /// Created ExternalSecret/ClusterExternalSecret names
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_secrets: Vec<ExternalSecretRef>,

    /// Last sync time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_time: Option<String>,

    /// Conditions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<ExternalSecretCondition>,

    /// Observed generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum ExternalSecretPhase {
    #[default]
    Pending,
    Creating,
    Ready,
    Failed,
    Deleting,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSecretRef {
    /// Name of the ExternalSecret
    pub name: String,

    /// Namespace (for namespace-scoped)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    /// Sync status
    pub status: SecretSyncStatus,

    /// Target K8s secret name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_secret: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum SecretSyncStatus {
    #[default]
    Pending,
    Synced,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExternalSecretCondition {
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
