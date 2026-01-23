//! AppDefinition - Minimal resource configuration for applications
//!
//! A single YAML file that defines everything needed to run an application:
//! - Container configuration
//! - Kubernetes resources
//! - Port forwards for local development
//! - Security policies and scanning
//! - Observability configuration
//! - Repository metadata

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Complete application definition in a single file
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppDefinition {
    /// API version for the app definition schema
    #[serde(default = "default_api_version")]
    pub api_version: String,

    /// Kind identifier
    #[serde(default = "default_kind")]
    pub kind: String,

    /// Application metadata
    pub metadata: AppMetadata,

    /// Application specification
    pub spec: AppSpec,
}

fn default_api_version() -> String {
    "platform.yurikrupnik.com/v1".to_string()
}

fn default_kind() -> String {
    "AppDefinition".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppMetadata {
    /// Application name
    pub name: String,

    /// Namespace (defaults to app name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    /// Version/tag
    #[serde(default = "default_version")]
    pub version: String,

    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Team/owner
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,

    /// Labels
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,

    /// Annotations
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,
}

fn default_version() -> String {
    "latest".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppSpec {
    /// Container configuration
    pub container: ContainerSpec,

    /// Kubernetes resources (minimal)
    #[serde(default)]
    pub resources: ResourceSpec,

    /// Port forwards for local development
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub port_forwards: Vec<PortForwardSpec>,

    /// Dependencies on other services
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<DependencySpec>,

    /// Security configuration
    #[serde(default)]
    pub security: SecuritySpec,

    /// Observability configuration
    #[serde(default)]
    pub observability: ObservabilitySpec,

    /// Repository configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<RepositorySpec>,

    /// FinOps configuration
    #[serde(default)]
    pub finops: FinOpsSpec,

    /// Environment-specific overrides
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environments: BTreeMap<String, EnvironmentOverride>,
}

// ============ Container Spec ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContainerSpec {
    /// Container image (required)
    pub image: String,

    /// Image pull policy
    #[serde(default)]
    pub pull_policy: PullPolicy,

    /// Container ports
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<ContainerPort>,

    /// Environment variables
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<EnvVar>,

    /// Environment from ConfigMaps/Secrets
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_from: Vec<EnvFromSource>,

    /// Volume mounts
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volume_mounts: Vec<VolumeMount>,

    /// Command override
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command: Vec<String>,

    /// Args override
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,

    /// Health checks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthChecks>,

    /// Resource limits/requests
    #[serde(default)]
    pub resources: ContainerResources,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PullPolicy {
    Always,
    #[default]
    IfNotPresent,
    Never,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContainerPort {
    /// Port name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Container port
    pub port: u16,
    /// Protocol (TCP/UDP)
    #[serde(default = "default_tcp")]
    pub protocol: String,
}

fn default_tcp() -> String {
    "TCP".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    /// Variable name
    pub name: String,
    /// Direct value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Value from reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_from: Option<EnvVarSource>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum EnvVarSource {
    /// From Secret
    SecretKeyRef { name: String, key: String },
    /// From ConfigMap
    ConfigMapKeyRef { name: String, key: String },
    /// From field (e.g., metadata.name)
    FieldRef { field_path: String },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvFromSource {
    /// ConfigMap name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_map_ref: Option<String>,
    /// Secret name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<String>,
    /// Prefix for env vars
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VolumeMount {
    /// Mount path in container
    pub mount_path: String,
    /// Volume name
    pub name: String,
    /// Read-only
    #[serde(default)]
    pub read_only: bool,
    /// Sub-path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthChecks {
    /// Liveness probe
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liveness: Option<Probe>,
    /// Readiness probe
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readiness: Option<Probe>,
    /// Startup probe
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup: Option<Probe>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    /// HTTP GET check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_get: Option<HttpProbe>,
    /// TCP socket check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcp_socket: Option<TcpProbe>,
    /// Exec check
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec: Option<ExecProbe>,
    /// Initial delay seconds
    #[serde(default = "default_initial_delay")]
    pub initial_delay_seconds: u32,
    /// Period seconds
    #[serde(default = "default_period")]
    pub period_seconds: u32,
    /// Timeout seconds
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u32,
    /// Failure threshold
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
}

fn default_initial_delay() -> u32 { 5 }
fn default_period() -> u32 { 10 }
fn default_timeout() -> u32 { 3 }
fn default_failure_threshold() -> u32 { 3 }

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HttpProbe {
    pub path: String,
    pub port: u16,
    #[serde(default = "default_http")]
    pub scheme: String,
}

fn default_http() -> String { "HTTP".to_string() }

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct TcpProbe {
    pub port: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct ExecProbe {
    pub command: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContainerResources {
    /// CPU request (e.g., "100m", "0.5")
    #[serde(default = "default_cpu_request")]
    pub cpu_request: String,
    /// Memory request (e.g., "128Mi", "1Gi")
    #[serde(default = "default_memory_request")]
    pub memory_request: String,
    /// CPU limit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_limit: Option<String>,
    /// Memory limit
    #[serde(default = "default_memory_limit")]
    pub memory_limit: String,
    /// GPU request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu: Option<GpuRequest>,
}

fn default_cpu_request() -> String { "100m".to_string() }
fn default_memory_request() -> String { "128Mi".to_string() }
fn default_memory_limit() -> String { "256Mi".to_string() }

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GpuRequest {
    /// GPU type (nvidia.com/gpu, amd.com/gpu)
    #[serde(default = "default_gpu_type")]
    pub resource_type: String,
    /// Number of GPUs
    pub count: u32,
}

fn default_gpu_type() -> String { "nvidia.com/gpu".to_string() }

// ============ Resource Spec ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSpec {
    /// Replicas
    #[serde(default = "default_replicas")]
    pub replicas: u32,

    /// Autoscaling configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autoscaling: Option<AutoscalingSpec>,

    /// Service configuration
    #[serde(default)]
    pub service: ServiceSpec,

    /// Ingress configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingress: Option<IngressSpec>,

    /// Volumes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volumes: Vec<VolumeSpec>,

    /// Service account
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account: Option<ServiceAccountSpec>,

    /// Pod disruption budget
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pdb: Option<PdbSpec>,

    /// Topology spread constraints
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub topology_spread: Vec<TopologySpreadSpec>,
}

fn default_replicas() -> u32 { 1 }

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AutoscalingSpec {
    pub min_replicas: u32,
    pub max_replicas: u32,
    #[serde(default = "default_cpu_target")]
    pub target_cpu_percent: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_memory_percent: Option<u32>,
    /// KEDA triggers
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keda_triggers: Vec<KedaTrigger>,
}

fn default_cpu_target() -> u32 { 80 }

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KedaTrigger {
    pub trigger_type: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSpec {
    /// Service type
    #[serde(default)]
    pub service_type: ServiceType,
    /// Port mappings
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<ServicePort>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ServiceType {
    #[default]
    ClusterIP,
    NodePort,
    LoadBalancer,
    None,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServicePort {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_port: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IngressSpec {
    /// Hostname
    pub host: String,
    /// TLS enabled
    #[serde(default = "default_true")]
    pub tls: bool,
    /// TLS secret name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_secret: Option<String>,
    /// Ingress class
    #[serde(default = "default_ingress_class")]
    pub ingress_class: String,
    /// Path rules
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<IngressPath>,
    /// Annotations
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,
}

fn default_ingress_class() -> String { "nginx".to_string() }

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IngressPath {
    pub path: String,
    #[serde(default = "default_path_type")]
    pub path_type: String,
    pub service_port: u16,
}

fn default_path_type() -> String { "Prefix".to_string() }

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum VolumeSpec {
    /// Empty dir
    EmptyDir { name: String, medium: Option<String> },
    /// ConfigMap
    ConfigMap { name: String, config_map_name: String },
    /// Secret
    Secret { name: String, secret_name: String },
    /// PVC
    Pvc { name: String, claim_name: String },
    /// PVC template (StatefulSet)
    PvcTemplate {
        name: String,
        storage_class: Option<String>,
        size: String,
        access_modes: Vec<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceAccountSpec {
    /// Create new service account
    #[serde(default = "default_true")]
    pub create: bool,
    /// Service account name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// IRSA role ARN (AWS)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub irsa_role_arn: Option<String>,
    /// Workload Identity SA (GCP)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workload_identity_sa: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PdbSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_available: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_unavailable: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TopologySpreadSpec {
    pub max_skew: u32,
    pub topology_key: String,
    #[serde(default = "default_when_unsatisfiable")]
    pub when_unsatisfiable: String,
}

fn default_when_unsatisfiable() -> String { "ScheduleAnyway".to_string() }

// ============ Port Forwards ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PortForwardSpec {
    /// Name for this forward
    pub name: String,
    /// Local port
    pub local_port: u16,
    /// Remote port (in cluster)
    pub remote_port: u16,
    /// Target type
    #[serde(default)]
    pub target_type: PortForwardTarget,
    /// Target name (service/pod name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PortForwardTarget {
    #[default]
    Service,
    Pod,
    Deployment,
}

// ============ Dependencies ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DependencySpec {
    /// Dependency name
    pub name: String,
    /// Dependency type
    #[serde(rename = "type")]
    pub dep_type: DependencyType,
    /// Version constraint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Required or optional
    #[serde(default = "default_true")]
    pub required: bool,
    /// Connection string env var
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_env: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    Postgres,
    Redis,
    MongoDB,
    Kafka,
    RabbitMQ,
    Elasticsearch,
    S3,
    GCS,
    Service,
    External,
}

// ============ Security ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecuritySpec {
    /// Pod security standards level
    #[serde(default)]
    pub pod_security_level: PodSecurityLevel,

    /// Network policies
    #[serde(default)]
    pub network_policy: NetworkPolicySpec,

    /// Container security context
    #[serde(default)]
    pub security_context: SecurityContextSpec,

    /// Image scanning configuration
    #[serde(default)]
    pub image_scanning: ImageScanningSpec,

    /// Runtime security (Falco, etc.)
    #[serde(default)]
    pub runtime_security: RuntimeSecuritySpec,

    /// Secrets management
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<SecretsManagementSpec>,

    /// RBAC rules
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rbac_rules: Vec<RbacRule>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PodSecurityLevel {
    Privileged,
    #[default]
    Baseline,
    Restricted,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPolicySpec {
    /// Enable network policy
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Allowed ingress sources
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ingress_from: Vec<NetworkPeer>,
    /// Allowed egress destinations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub egress_to: Vec<NetworkPeer>,
    /// Default deny all
    #[serde(default)]
    pub default_deny: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum NetworkPeer {
    /// Namespace selector
    Namespace { labels: BTreeMap<String, String> },
    /// Pod selector
    Pod { labels: BTreeMap<String, String> },
    /// CIDR block
    Cidr { cidr: String },
    /// Allow from same namespace
    SameNamespace,
    /// Allow from any
    Any,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecurityContextSpec {
    /// Run as non-root
    #[serde(default = "default_true")]
    pub run_as_non_root: bool,
    /// Run as user ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_as_user: Option<i64>,
    /// Run as group ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_as_group: Option<i64>,
    /// Read-only root filesystem
    #[serde(default = "default_true")]
    pub read_only_root_filesystem: bool,
    /// Allow privilege escalation
    #[serde(default)]
    pub allow_privilege_escalation: bool,
    /// Drop capabilities
    #[serde(default = "default_drop_all")]
    pub drop_capabilities: Vec<String>,
    /// Add capabilities
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_capabilities: Vec<String>,
    /// Seccomp profile
    #[serde(default = "default_seccomp")]
    pub seccomp_profile: String,
}

fn default_drop_all() -> Vec<String> { vec!["ALL".to_string()] }
fn default_seccomp() -> String { "RuntimeDefault".to_string() }

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImageScanningSpec {
    /// Enable image scanning
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Scanner to use
    #[serde(default)]
    pub scanner: ImageScanner,
    /// Fail on critical vulnerabilities
    #[serde(default = "default_true")]
    pub fail_on_critical: bool,
    /// Fail on high vulnerabilities
    #[serde(default)]
    pub fail_on_high: bool,
    /// Ignore unfixed vulnerabilities
    #[serde(default)]
    pub ignore_unfixed: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ImageScanner {
    #[default]
    Trivy,
    Grype,
    Snyk,
    Clair,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSecuritySpec {
    /// Enable Falco rules
    #[serde(default)]
    pub falco_enabled: bool,
    /// Custom Falco rules
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub falco_rules: Vec<String>,
    /// Enable seccomp profiling
    #[serde(default)]
    pub seccomp_profiling: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretsManagementSpec {
    /// Provider
    pub provider: SecretsProvider,
    /// Store name
    pub store: String,
    /// Secrets to sync
    pub secrets: Vec<SecretMapping>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SecretsProvider {
    Vault,
    Aws,
    Gcp,
    Azure,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretMapping {
    /// Remote key/path
    pub remote_ref: String,
    /// K8s secret key
    pub secret_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RbacRule {
    pub api_groups: Vec<String>,
    pub resources: Vec<String>,
    pub verbs: Vec<String>,
}

// ============ Observability ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ObservabilitySpec {
    /// Metrics configuration
    #[serde(default)]
    pub metrics: MetricsSpec,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingSpec,

    /// Tracing configuration
    #[serde(default)]
    pub tracing: TracingSpec,

    /// Alerting rules
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alerts: Vec<AlertRule>,

    /// Dashboards
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dashboards: Vec<DashboardSpec>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSpec {
    /// Enable Prometheus scraping
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Metrics path
    #[serde(default = "default_metrics_path")]
    pub path: String,
    /// Metrics port
    #[serde(default = "default_metrics_port")]
    pub port: u16,
    /// Scrape interval
    #[serde(default = "default_scrape_interval")]
    pub scrape_interval: String,
    /// ServiceMonitor labels
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
}

fn default_metrics_path() -> String { "/metrics".to_string() }
fn default_metrics_port() -> u16 { 9090 }
fn default_scrape_interval() -> String { "30s".to_string() }

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoggingSpec {
    /// Log format
    #[serde(default)]
    pub format: LogFormat,
    /// Log level
    #[serde(default)]
    pub level: LogLevel,
    /// Send to Loki
    #[serde(default)]
    pub loki_enabled: bool,
    /// Structured logging fields
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Json,
    Text,
    Logfmt,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TracingSpec {
    /// Enable tracing
    #[serde(default)]
    pub enabled: bool,
    /// OTLP endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otlp_endpoint: Option<String>,
    /// Sample rate (0.0 - 1.0)
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f64,
    /// Service name override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_name: Option<String>,
}

fn default_sample_rate() -> f64 { 0.1 }

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlertRule {
    /// Alert name
    pub name: String,
    /// PromQL expression
    pub expr: String,
    /// Duration
    #[serde(rename = "for")]
    pub duration: String,
    /// Severity
    pub severity: AlertSeverity,
    /// Summary
    pub summary: String,
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSpec {
    /// Dashboard name
    pub name: String,
    /// Grafana folder
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// Dashboard JSON (inline or path)
    pub json: String,
}

// ============ Repository ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySpec {
    /// Git URL
    pub url: String,
    /// Default branch
    #[serde(default = "default_branch")]
    pub branch: String,
    /// Path in repo
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// CI/CD configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci: Option<CiSpec>,
    /// Code owners
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owners: Vec<String>,
    /// SCA (Software Composition Analysis)
    #[serde(default)]
    pub sca: ScaSpec,
    /// SAST (Static Application Security Testing)
    #[serde(default)]
    pub sast: SastSpec,
}

fn default_branch() -> String { "main".to_string() }

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CiSpec {
    /// CI provider
    pub provider: CiProvider,
    /// Pipeline file path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_path: Option<String>,
    /// Required checks
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_checks: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CiProvider {
    GitHub,
    GitLab,
    CircleCI,
    Jenkins,
    ArgoWorkflows,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScaSpec {
    /// Enable dependency scanning
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Scanner
    #[serde(default)]
    pub scanner: ScaScanner,
    /// Fail on critical
    #[serde(default = "default_true")]
    pub fail_on_critical: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScaScanner {
    #[default]
    Dependabot,
    Snyk,
    Renovate,
    Trivy,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SastSpec {
    /// Enable SAST scanning
    #[serde(default)]
    pub enabled: bool,
    /// Scanner
    #[serde(default)]
    pub scanner: SastScanner,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SastScanner {
    #[default]
    Semgrep,
    CodeQL,
    Sonar,
}

// ============ FinOps ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FinOpsSpec {
    /// Cost center / billing tag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_center: Option<String>,

    /// Budget alert (monthly USD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly_budget: Option<f64>,

    /// Resource recommendations enabled
    #[serde(default = "default_true")]
    pub recommendations_enabled: bool,

    /// Spot/preemptible tolerance
    #[serde(default)]
    pub spot_tolerance: SpotTolerance,

    /// Reserved capacity eligible
    #[serde(default)]
    pub reserved_eligible: bool,

    /// Cost allocation labels
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cost_labels: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SpotTolerance {
    #[default]
    None,
    Low,
    Medium,
    High,
}

// ============ Environment Overrides ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentOverride {
    /// Override replicas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<u32>,

    /// Override resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ContainerResources>,

    /// Override env vars
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<EnvVar>,

    /// Override autoscaling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autoscaling: Option<AutoscalingSpec>,

    /// Override ingress host
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingress_host: Option<String>,
}

// ============ Helper Functions ============

fn default_true() -> bool { true }

impl AppDefinition {
    /// Load from YAML file
    pub fn from_yaml(content: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(content)
    }

    /// Save to YAML
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// Get effective namespace
    pub fn namespace(&self) -> &str {
        self.metadata.namespace.as_deref().unwrap_or(&self.metadata.name)
    }
}
