//! Cluster CRD - Multi-cloud Kubernetes cluster abstraction
//!
//! Provisions GKE, EKS, or AKS clusters via Crossplane with Vault-based
//! authentication for cloud credentials.

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Multi-cloud Kubernetes Cluster CRD
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "Cluster",
    namespaced,
    status = "ClusterStatus",
    shortname = "cl",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Provider", "type":"string", "jsonPath":".spec.provider"}"#,
    printcolumn = r#"{"name":"Region", "type":"string", "jsonPath":".spec.parameters.region"}"#,
    printcolumn = r#"{"name":"Version", "type":"string", "jsonPath":".spec.parameters.kubernetesVersion"}"#,
    printcolumn = r#"{"name":"Ready", "type":"boolean", "jsonPath":".status.ready"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct ClusterSpec {
    /// Cloud provider: gcp, aws, or azure
    pub provider: ClusterCloudProvider,

    /// Provider-agnostic cluster parameters
    pub parameters: ClusterParameters,

    /// Node pool configurations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_pools: Vec<NodePoolSpec>,

    /// Network configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<ClusterNetworkConfig>,

    /// Vault-based authentication configuration
    pub vault_auth: VaultCloudAuth,

    /// Addons to install (monitoring, logging, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addons: Option<ClusterAddons>,

    /// Deletion policy: Delete or Orphan the cloud resources
    #[serde(default)]
    pub deletion_policy: ClusterDeletionPolicy,

    /// Labels to apply to cluster and resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,

    /// Where to write connection details (kubeconfig, endpoint, CA)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_connection_secret_to_ref: Option<ClusterConnectionSecretRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ClusterCloudProvider {
    #[default]
    Gcp,
    Aws,
    Azure,
}

// ============ Cluster Parameters ============

/// Provider-agnostic cluster parameters with provider-specific overrides
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterParameters {
    /// Kubernetes version (e.g., "1.29", "1.28.5-gke.1200")
    pub kubernetes_version: String,

    /// Region/location for the cluster
    pub region: String,

    /// Cluster name (if different from metadata.name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_name: Option<String>,

    /// Control plane configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_plane: Option<ControlPlaneConfig>,

    /// Maintenance window configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance_window: Option<MaintenanceWindow>,

    /// Auto-upgrade enabled for control plane
    #[serde(default = "default_true")]
    pub auto_upgrade: bool,

    /// Enable workload identity (GKE) / IRSA (EKS) / Managed Identity (AKS)
    #[serde(default = "default_true")]
    pub workload_identity_enabled: bool,

    /// Enable network policies
    #[serde(default = "default_true")]
    pub network_policy_enabled: bool,

    /// Enable secret encryption with KMS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets_encryption: Option<SecretsEncryption>,

    /// GCP-specific settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gcp: Option<GcpClusterConfig>,

    /// AWS-specific settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aws: Option<AwsClusterConfig>,

    /// Azure-specific settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure: Option<AzureClusterConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ControlPlaneConfig {
    /// High availability (regional cluster)
    #[serde(default = "default_true")]
    pub high_availability: bool,

    /// Authorized networks for API access
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authorized_networks: Vec<AuthorizedNetwork>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedNetwork {
    pub name: String,
    pub cidr_block: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceWindow {
    /// Start time in RFC3339 format
    pub start_time: String,
    /// Duration in hours
    pub duration_hours: i32,
    /// Recurrence rule (RRULE format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretsEncryption {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kms_key_id: Option<String>,
}

// ============ GCP-Specific Configuration ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GcpClusterConfig {
    /// GCP project ID
    pub project_id: String,

    /// Enable Autopilot mode
    #[serde(default)]
    pub autopilot: bool,

    /// Release channel: RAPID, REGULAR, STABLE
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_channel: Option<GkeReleaseChannel>,

    /// Binary Authorization configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_authorization: Option<BinaryAuthorizationConfig>,

    /// Enable Dataplane V2 (cilium-based)
    #[serde(default = "default_true")]
    pub dataplane_v2: bool,

    /// VPC-native cluster (alias IPs)
    #[serde(default = "default_true")]
    pub vpc_native: bool,

    /// Workload Identity pool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workload_identity_pool: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GkeReleaseChannel {
    Rapid,
    Regular,
    Stable,
    Unspecified,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BinaryAuthorizationConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation_mode: Option<String>,
}

// ============ AWS-Specific Configuration ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AwsClusterConfig {
    /// IAM role ARN for the cluster
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_arn: Option<String>,

    /// Enable Fargate profiles
    #[serde(default)]
    pub fargate_enabled: bool,

    /// Fargate profiles
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fargate_profiles: Vec<FargateProfile>,

    /// EKS Add-ons
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eks_addons: Vec<EksAddon>,

    /// Create OIDC provider for IRSA
    #[serde(default = "default_true")]
    pub create_oidc_provider: bool,

    /// Endpoint access configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_access: Option<EksEndpointAccess>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FargateProfile {
    pub name: String,
    pub selectors: Vec<FargateSelector>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pod_execution_role_arn: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FargateSelector {
    pub namespace: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EksAddon {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account_role_arn: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EksEndpointAccess {
    pub private_access: bool,
    pub public_access: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub public_access_cidrs: Vec<String>,
}

// ============ Azure-Specific Configuration ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AzureClusterConfig {
    /// Azure resource group
    pub resource_group: String,

    /// Azure subscription ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<String>,

    /// DNS prefix for the cluster
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns_prefix: Option<String>,

    /// SKU tier: Free or Standard
    #[serde(default)]
    pub sku_tier: AksSKUTier,

    /// Azure AD integration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure_ad_config: Option<AzureAdConfig>,

    /// Identity type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_type: Option<AksIdentityType>,

    /// User-assigned identity resource ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_assigned_identity_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum AksSKUTier {
    #[default]
    Free,
    Standard,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum AksIdentityType {
    SystemAssigned,
    UserAssigned,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AzureAdConfig {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub admin_group_object_ids: Vec<String>,
    #[serde(default)]
    pub azure_rbac_enabled: bool,
}

// ============ Node Pool Configuration ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodePoolSpec {
    /// Node pool name
    pub name: String,

    /// Machine/instance type
    pub machine_type: String,

    /// Disk size in GB
    #[serde(default = "default_disk_size")]
    pub disk_size_gb: i32,

    /// Disk type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_type: Option<NodeDiskType>,

    /// Node count (for fixed size pools)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_count: Option<i32>,

    /// Autoscaling configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autoscaling: Option<NodePoolAutoscaling>,

    /// Spot/preemptible instances
    #[serde(default)]
    pub spot: bool,

    /// Node taints
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub taints: Vec<NodeTaint>,

    /// Node labels
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,

    /// Enable auto-upgrade
    #[serde(default = "default_true")]
    pub auto_upgrade: bool,

    /// Enable auto-repair (GKE/AKS)
    #[serde(default = "default_true")]
    pub auto_repair: bool,

    /// Max pods per node
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_pods_per_node: Option<i32>,

    /// System pool (AKS-specific)
    #[serde(default)]
    pub system_pool: bool,
}

fn default_disk_size() -> i32 {
    100
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NodeDiskType {
    Ssd,
    Standard,
    Balanced,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodePoolAutoscaling {
    pub enabled: bool,
    pub min_count: i32,
    pub max_count: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeTaint {
    pub key: String,
    pub value: String,
    pub effect: TaintEffect,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum TaintEffect {
    NoSchedule,
    PreferNoSchedule,
    NoExecute,
}

// ============ Network Configuration ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterNetworkConfig {
    /// Private cluster (no public endpoint)
    #[serde(default)]
    pub private_cluster: bool,

    /// Master/control plane authorized CIDR blocks
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub master_authorized_cidr_blocks: Vec<String>,

    /// VPC/VNet configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpc: Option<VpcConfig>,

    /// Pod CIDR range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pod_cidr: Option<String>,

    /// Service CIDR range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_cidr: Option<String>,

    /// Enable private nodes (no public IPs)
    #[serde(default = "default_true")]
    pub private_nodes: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VpcConfig {
    /// Use existing VPC
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpc_id: Option<String>,

    /// Subnet IDs (for AWS/Azure)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subnet_ids: Vec<String>,

    /// Create new VPC
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_vpc: Option<VpcCreateConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VpcCreateConfig {
    pub cidr_block: String,
    #[serde(default)]
    pub enable_nat_gateway: bool,
    #[serde(default)]
    pub single_nat_gateway: bool,
}

// ============ Cluster Addons ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterAddons {
    /// Enable monitoring
    #[serde(default = "default_true")]
    pub monitoring: bool,

    /// Enable logging
    #[serde(default = "default_true")]
    pub logging: bool,

    /// HTTP load balancing / Ingress controller
    #[serde(default = "default_true")]
    pub http_load_balancing: bool,

    /// Network policy provider
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_policy_provider: Option<NetworkPolicyProvider>,

    /// DNS autoscaling
    #[serde(default)]
    pub dns_autoscaling: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkPolicyProvider {
    Calico,
    Cilium,
    Azure,
}

// ============ Vault Authentication ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultCloudAuth {
    /// Authentication mode: dynamic (secrets engine) or static (KV)
    pub mode: VaultAuthMode,

    /// Vault server address
    pub server: String,

    /// Vault authentication method
    pub vault_auth: VaultAuthMethod,

    /// Dynamic secrets engine configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<VaultDynamicConfig>,

    /// Static KV secrets configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_secrets: Option<VaultStaticConfig>,

    /// TTL for credentials (dynamic mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,

    /// CA certificate for Vault TLS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ca_cert: Option<ClusterSecretKeySelector>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum VaultAuthMode {
    Dynamic,
    Static,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultAuthMethod {
    /// Token authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<VaultTokenAuth>,

    /// Kubernetes authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kubernetes: Option<VaultKubernetesAuthConfig>,

    /// AppRole authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_role: Option<VaultAppRoleAuthConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultTokenAuth {
    pub secret_ref: ClusterSecretKeySelector,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultKubernetesAuthConfig {
    /// Mount path (e.g., "kubernetes")
    pub mount_path: String,
    /// Vault role
    pub role: String,
    /// Service account for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account_ref: Option<ClusterServiceAccountRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultAppRoleAuthConfig {
    /// Mount path (e.g., "approle")
    pub mount_path: String,
    /// Role ID secret reference
    pub role_id: ClusterSecretKeySelector,
    /// Secret ID secret reference
    pub secret_id: ClusterSecretKeySelector,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultDynamicConfig {
    /// Secrets engine path (e.g., "gcp", "aws", "azure")
    pub secrets_engine_path: String,
    /// Role name in the secrets engine
    pub role: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultStaticConfig {
    /// KV mount path (e.g., "secret" or "kv")
    pub kv_mount_path: String,

    /// KV version (v1 or v2)
    #[serde(default = "default_kv_version")]
    pub kv_version: String,

    /// Secret path within KV
    pub secret_path: String,

    /// Key mappings for credentials
    pub keys: VaultStaticKeys,
}

fn default_kv_version() -> String {
    "v2".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultStaticKeys {
    /// GCP: service account JSON key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gcp_credentials: Option<String>,

    /// AWS: access key ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aws_access_key_id: Option<String>,

    /// AWS: secret access key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aws_secret_access_key: Option<String>,

    /// Azure: client ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure_client_id: Option<String>,

    /// Azure: client secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure_client_secret: Option<String>,

    /// Azure: tenant ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure_tenant_id: Option<String>,

    /// Azure: subscription ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure_subscription_id: Option<String>,
}

// ============ Shared Types ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum ClusterDeletionPolicy {
    #[default]
    Delete,
    Orphan,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterConnectionSecretRef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterSecretKeySelector {
    pub name: String,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterServiceAccountRef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

fn default_true() -> bool {
    true
}

// ============ Status ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterStatus {
    /// Current phase
    #[serde(default)]
    pub phase: ClusterPhase,

    /// Whether the cluster is ready
    #[serde(default)]
    pub ready: bool,

    /// Synced with cloud provider
    #[serde(default)]
    pub synced: bool,

    /// Cluster endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,

    /// Cluster CA certificate (base64 encoded)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ca_certificate: Option<String>,

    /// Cloud provider cluster ID/ARN/resource ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_id: Option<String>,

    /// Kubernetes version running
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kubernetes_version: Option<String>,

    /// Node pool statuses
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_pools: Vec<NodePoolStatus>,

    /// Managed Crossplane resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub managed_resources: Option<ManagedClusterResources>,

    /// Vault credential status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vault_credential_status: Option<VaultCredentialStatus>,

    /// Conditions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<ClusterCondition>,

    /// Observed generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,

    /// Last reconciliation time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reconcile_time: Option<String>,

    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum ClusterPhase {
    #[default]
    Pending,
    FetchingCredentials,
    CreatingProviderConfig,
    CreatingCluster,
    CreatingNodePools,
    Ready,
    Failed,
    Deleting,
    Updating,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodePoolStatus {
    pub name: String,
    pub ready: bool,
    pub node_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManagedClusterResources {
    /// ProviderConfig name
    pub provider_config: String,
    /// Cluster resource reference
    pub cluster: ManagedResourceRef,
    /// Node pool resource references
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_pools: Vec<ManagedResourceRef>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManagedResourceRef {
    pub api_version: String,
    pub kind: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VaultCredentialStatus {
    /// Credential source (dynamic or static)
    pub source: String,
    /// Last credential refresh time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_refresh_time: Option<String>,
    /// Credential expiry time (for dynamic)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// Secret name where credentials are stored
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_secret: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterCondition {
    #[serde(rename = "type")]
    pub condition_type: String,
    pub status: String,
    pub reason: String,
    pub message: String,
    pub last_transition_time: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        assert_eq!(ClusterCloudProvider::default(), ClusterCloudProvider::Gcp);
        assert_eq!(ClusterDeletionPolicy::default(), ClusterDeletionPolicy::Delete);
        assert_eq!(ClusterPhase::default(), ClusterPhase::Pending);
        assert!(default_true());
        assert_eq!(default_disk_size(), 100);
        assert_eq!(default_kv_version(), "v2");
    }
}
