//! Dependency Checker for Platform Operator
//!
//! Validates that required operators and CRDs are available before reconciling.
//! This prevents controllers from failing when dependencies aren't installed.

use kube::{
    api::{Api, DynamicObject, GroupVersionKind},
    discovery::{ApiCapabilities, ApiResource, Scope},
    Client,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Dependency definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dependency {
    /// Human-readable name
    pub name: String,
    /// API Group (e.g., "source.toolkit.fluxcd.io")
    pub group: String,
    /// API Version (e.g., "v1")
    pub version: String,
    /// Kind (e.g., "GitRepository")
    pub kind: String,
    /// Whether this dependency is required or optional
    pub required: bool,
    /// Description for status reporting
    pub description: String,
    /// Installation hint for users
    pub install_hint: Option<String>,
}

impl Dependency {
    pub fn new(name: &str, group: &str, version: &str, kind: &str) -> Self {
        Self {
            name: name.to_string(),
            group: group.to_string(),
            version: version.to_string(),
            kind: kind.to_string(),
            required: true,
            description: format!("{} ({}/{})", kind, group, version),
            install_hint: None,
        }
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn with_hint(mut self, hint: &str) -> Self {
        self.install_hint = Some(hint.to_string());
        self
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }
}

/// Status of a dependency check
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DependencyStatus {
    /// Dependency is available and ready
    Available,
    /// Dependency CRD exists but operator may not be running
    CrdOnly,
    /// Dependency is not installed
    Missing,
    /// Check failed (network error, etc.)
    Unknown(String),
}

/// Result of a dependency check
#[derive(Clone, Debug)]
pub struct DependencyCheckResult {
    pub dependency: Dependency,
    pub status: DependencyStatus,
    pub checked_at: Instant,
}

impl DependencyCheckResult {
    pub fn is_available(&self) -> bool {
        matches!(self.status, DependencyStatus::Available | DependencyStatus::CrdOnly)
    }
}

/// Cache entry for dependency checks
struct CacheEntry {
    status: DependencyStatus,
    checked_at: Instant,
}

/// Dependency checker with caching
pub struct DependencyChecker {
    client: Client,
    /// Cache of dependency check results (TTL-based)
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Cache TTL (how long to cache results)
    cache_ttl: Duration,
}

impl DependencyChecker {
    /// Create a new dependency checker
    pub fn new(client: Client) -> Self {
        Self {
            client,
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(60), // Cache for 1 minute
        }
    }

    /// Create with custom cache TTL
    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Generate cache key for a dependency
    fn cache_key(group: &str, version: &str, kind: &str) -> String {
        format!("{}/{}/{}", group, version, kind)
    }

    /// Check if a specific API resource is available
    pub async fn is_available(&self, group: &str, version: &str, kind: &str) -> bool {
        self.check_api(group, version, kind).await == DependencyStatus::Available
            || self.check_api(group, version, kind).await == DependencyStatus::CrdOnly
    }

    /// Check API availability with caching
    pub async fn check_api(&self, group: &str, version: &str, kind: &str) -> DependencyStatus {
        let key = Self::cache_key(group, version, kind);

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(&key) {
                if entry.checked_at.elapsed() < self.cache_ttl {
                    debug!("Cache hit for {}", key);
                    return entry.status.clone();
                }
            }
        }

        // Perform the check
        let status = self.check_api_uncached(group, version, kind).await;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                key,
                CacheEntry {
                    status: status.clone(),
                    checked_at: Instant::now(),
                },
            );
        }

        status
    }

    /// Check API without caching
    async fn check_api_uncached(&self, group: &str, version: &str, kind: &str) -> DependencyStatus {
        let gvk = GroupVersionKind {
            group: group.to_string(),
            version: version.to_string(),
            kind: kind.to_string(),
        };

        // Try to discover the API resource
        let api_resource = ApiResource::from_gvk(&gvk);
        let api: Api<DynamicObject> = Api::all_with(self.client.clone(), &api_resource);

        // Try to list (with limit 1) to verify the API is working
        match api.list(&kube::api::ListParams::default().limit(1)).await {
            Ok(_) => {
                debug!("Dependency available: {}/{}/{}", group, version, kind);
                DependencyStatus::Available
            }
            Err(kube::Error::Api(err)) if err.code == 404 => {
                debug!("Dependency missing (404): {}/{}/{}", group, version, kind);
                DependencyStatus::Missing
            }
            Err(kube::Error::Api(err)) if err.code == 403 => {
                // CRD exists but we don't have permission (operator not configured)
                debug!(
                    "Dependency CRD exists but forbidden: {}/{}/{}",
                    group, version, kind
                );
                DependencyStatus::CrdOnly
            }
            Err(e) => {
                warn!(
                    "Dependency check failed for {}/{}/{}: {}",
                    group, version, kind, e
                );
                DependencyStatus::Unknown(e.to_string())
            }
        }
    }

    /// Check a dependency
    pub async fn check(&self, dep: &Dependency) -> DependencyCheckResult {
        let status = self.check_api(&dep.group, &dep.version, &dep.kind).await;
        DependencyCheckResult {
            dependency: dep.clone(),
            status,
            checked_at: Instant::now(),
        }
    }

    /// Check multiple dependencies
    pub async fn check_all(&self, deps: &[Dependency]) -> Vec<DependencyCheckResult> {
        let mut results = Vec::with_capacity(deps.len());
        for dep in deps {
            results.push(self.check(dep).await);
        }
        results
    }

    /// Check if all required dependencies are available
    pub async fn all_required_available(&self, deps: &[Dependency]) -> bool {
        for dep in deps {
            if dep.required {
                let result = self.check(dep).await;
                if !result.is_available() {
                    return false;
                }
            }
        }
        true
    }

    /// Get missing dependencies
    pub async fn get_missing(&self, deps: &[Dependency]) -> Vec<DependencyCheckResult> {
        let results = self.check_all(deps).await;
        results
            .into_iter()
            .filter(|r| !r.is_available() && r.dependency.required)
            .collect()
    }

    /// Clear the cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

/// Well-known dependencies for the platform operator
pub mod known_dependencies {
    use super::Dependency;

    /// FluxCD Source Controller
    pub fn fluxcd_source() -> Dependency {
        Dependency::new(
            "FluxCD Source Controller",
            "source.toolkit.fluxcd.io",
            "v1",
            "GitRepository",
        )
        .with_description("FluxCD Source Controller for GitOps workflows")
        .with_hint("Install FluxCD: flux install")
    }

    /// FluxCD Kustomize Controller
    pub fn fluxcd_kustomize() -> Dependency {
        Dependency::new(
            "FluxCD Kustomize Controller",
            "kustomize.toolkit.fluxcd.io",
            "v1",
            "Kustomization",
        )
        .with_description("FluxCD Kustomize Controller for GitOps deployments")
        .with_hint("Install FluxCD: flux install")
    }

    /// FluxCD Helm Controller
    pub fn fluxcd_helm() -> Dependency {
        Dependency::new(
            "FluxCD Helm Controller",
            "helm.toolkit.fluxcd.io",
            "v2",
            "HelmRelease",
        )
        .with_description("FluxCD Helm Controller for Helm releases")
        .with_hint("Install FluxCD: flux install")
    }

    /// Crossplane
    pub fn crossplane() -> Dependency {
        Dependency::new(
            "Crossplane",
            "apiextensions.crossplane.io",
            "v1",
            "CompositeResourceDefinition",
        )
        .with_description("Crossplane for multi-cloud resource management")
        .with_hint("Install Crossplane: helm install crossplane crossplane-stable/crossplane -n crossplane-system")
    }

    /// External Secrets Operator
    pub fn external_secrets() -> Dependency {
        Dependency::new(
            "External Secrets Operator",
            "external-secrets.io",
            "v1beta1",
            "ExternalSecret",
        )
        .with_description("External Secrets Operator for secret management")
        .with_hint("Install ESO: helm install external-secrets external-secrets/external-secrets -n external-secrets")
    }

    /// CloudNativePG
    pub fn cnpg() -> Dependency {
        Dependency::new(
            "CloudNativePG",
            "postgresql.cnpg.io",
            "v1",
            "Cluster",
        )
        .with_description("CloudNativePG for PostgreSQL provisioning")
        .with_hint("Install CNPG: kubectl apply -f https://raw.githubusercontent.com/cloudnative-pg/cloudnative-pg/main/releases/cnpg-1.22.0.yaml")
    }

    /// Percona MongoDB Operator
    pub fn percona_mongodb() -> Dependency {
        Dependency::new(
            "Percona MongoDB Operator",
            "psmdb.percona.com",
            "v1",
            "PerconaServerMongoDB",
        )
        .with_description("Percona MongoDB Operator for MongoDB provisioning")
        .with_hint("Install Percona: kubectl apply -f https://raw.githubusercontent.com/percona/percona-server-mongodb-operator/main/deploy/bundle.yaml")
        .optional()
    }

    /// MongoDB Community Operator
    pub fn mongodb_community() -> Dependency {
        Dependency::new(
            "MongoDB Community Operator",
            "mongodbcommunity.mongodb.com",
            "v1",
            "MongoDBCommunity",
        )
        .with_description("MongoDB Community Operator")
        .with_hint("Install MongoDB Community Operator via Helm or kubectl")
        .optional()
    }

    /// Spotahome Redis Operator
    pub fn spotahome_redis() -> Dependency {
        Dependency::new(
            "Spotahome Redis Operator",
            "databases.spotahome.com",
            "v1",
            "RedisFailover",
        )
        .with_description("Spotahome Redis Operator for Redis HA")
        .with_hint("Install: kubectl apply -f https://raw.githubusercontent.com/spotahome/redis-operator/master/manifests/databases.spotahome.com_redisfailovers.yaml")
        .optional()
    }

    /// Dragonfly Operator
    pub fn dragonfly() -> Dependency {
        Dependency::new(
            "Dragonfly Operator",
            "dragonflydb.io",
            "v1alpha1",
            "Dragonfly",
        )
        .with_description("Dragonfly - Redis-compatible in-memory store")
        .with_hint("Install Dragonfly Operator via Helm")
        .optional()
    }

    /// KEDA (Kubernetes Event-driven Autoscaling)
    pub fn keda() -> Dependency {
        Dependency::new(
            "KEDA",
            "keda.sh",
            "v1alpha1",
            "ScaledObject",
        )
        .with_description("KEDA for event-driven autoscaling")
        .with_hint("Install KEDA: helm install keda kedacore/keda -n keda")
        .optional()
    }

    /// Cert-Manager
    pub fn cert_manager() -> Dependency {
        Dependency::new(
            "Cert-Manager",
            "cert-manager.io",
            "v1",
            "Certificate",
        )
        .with_description("Cert-Manager for TLS certificate management")
        .with_hint("Install cert-manager: kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.14.0/cert-manager.yaml")
        .optional()
    }

    /// Istio
    pub fn istio() -> Dependency {
        Dependency::new(
            "Istio",
            "networking.istio.io",
            "v1",
            "VirtualService",
        )
        .with_description("Istio service mesh")
        .with_hint("Install Istio: istioctl install")
        .optional()
    }

    /// All dependencies for GitOpsApp controller
    pub fn gitops_app_deps() -> Vec<Dependency> {
        vec![fluxcd_source(), fluxcd_kustomize()]
    }

    /// All dependencies for CrossplaneResource controller
    pub fn crossplane_resource_deps() -> Vec<Dependency> {
        vec![crossplane()]
    }

    /// All dependencies for ExternalSecretConfig controller
    pub fn external_secret_deps() -> Vec<Dependency> {
        vec![external_secrets()]
    }

    /// All dependencies for PostgresProvisioner controller
    pub fn postgres_provisioner_deps() -> Vec<Dependency> {
        vec![cnpg()]
    }

    /// All dependencies for MongoProvisioner controller (any of these)
    pub fn mongo_provisioner_deps() -> Vec<Dependency> {
        vec![
            percona_mongodb().optional(),
            mongodb_community().optional(),
        ]
    }

    /// All dependencies for RedisProvisioner controller (any of these)
    pub fn redis_provisioner_deps() -> Vec<Dependency> {
        vec![
            spotahome_redis().optional(),
            dragonfly().optional(),
        ]
    }

    /// Upbound AWS SES Provider
    pub fn upbound_ses() -> Dependency {
        Dependency::new(
            "Upbound AWS SES Provider",
            "ses.aws.upbound.io",
            "v1beta1",
            "DomainIdentity",
        )
        .with_description("Upbound Crossplane provider for AWS SES")
        .with_hint("Install: kubectl apply -f https://marketplace.upbound.io/providers/upbound/provider-aws-ses/v1.19.0/package.yaml")
    }

    /// Upbound AWS SNS Provider (for notifications)
    pub fn upbound_sns() -> Dependency {
        Dependency::new(
            "Upbound AWS SNS Provider",
            "sns.aws.upbound.io",
            "v1beta1",
            "Topic",
        )
        .with_description("Upbound Crossplane provider for AWS SNS")
        .with_hint("Install: kubectl apply -f https://marketplace.upbound.io/providers/upbound/provider-aws-sns/v1.19.0/package.yaml")
        .optional()
    }

    /// Upbound AWS IAM Provider (for IRSA)
    pub fn upbound_iam() -> Dependency {
        Dependency::new(
            "Upbound AWS IAM Provider",
            "iam.aws.upbound.io",
            "v1beta1",
            "Role",
        )
        .with_description("Upbound Crossplane provider for AWS IAM")
        .with_hint("Install: kubectl apply -f https://marketplace.upbound.io/providers/upbound/provider-aws-iam/v1.19.0/package.yaml")
    }

    /// All dependencies for EmailService controller
    pub fn email_service_deps() -> Vec<Dependency> {
        vec![
            crossplane(),
            upbound_ses(),
            upbound_iam(),
            upbound_sns().optional(),
        ]
    }

    /// Upbound AWS Bedrock Provider
    pub fn upbound_bedrock() -> Dependency {
        Dependency::new(
            "Upbound AWS Bedrock Provider",
            "bedrock.aws.upbound.io",
            "v1beta1",
            "Guardrail",
        )
        .with_description("Upbound Crossplane provider for AWS Bedrock")
        .with_hint("Install: kubectl apply -f https://marketplace.upbound.io/providers/upbound/provider-aws-bedrock/v1.19.0/package.yaml")
        .optional()
    }

    /// All dependencies for BedrockAccess controller
    pub fn bedrock_access_deps() -> Vec<Dependency> {
        vec![
            crossplane(),
            upbound_iam(),
            upbound_bedrock().optional(),
        ]
    }

    /// Upbound GCP Cloud Platform Provider (for Service Accounts, IAM)
    pub fn upbound_gcp_cloudplatform() -> Dependency {
        Dependency::new(
            "Upbound GCP Cloud Platform Provider",
            "cloudplatform.gcp.upbound.io",
            "v1beta1",
            "ServiceAccount",
        )
        .with_description("Upbound Crossplane provider for GCP Cloud Platform")
        .with_hint("Install: kubectl apply -f https://marketplace.upbound.io/providers/upbound/provider-gcp-cloudplatform/v1.0.0/package.yaml")
    }

    /// Upbound GCP Vertex AI Provider
    pub fn upbound_gcp_vertexai() -> Dependency {
        Dependency::new(
            "Upbound GCP Vertex AI Provider",
            "vertexai.gcp.upbound.io",
            "v1beta1",
            "Index",
        )
        .with_description("Upbound Crossplane provider for GCP Vertex AI")
        .with_hint("Install: kubectl apply -f https://marketplace.upbound.io/providers/upbound/provider-gcp-vertexai/v1.0.0/package.yaml")
        .optional()
    }

    /// All dependencies for VertexAIAccess controller
    pub fn vertex_ai_access_deps() -> Vec<Dependency> {
        vec![
            crossplane(),
            upbound_gcp_cloudplatform(),
            upbound_gcp_vertexai().optional(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_creation() {
        let dep = Dependency::new("Test", "example.com", "v1", "TestResource")
            .with_hint("Install with: kubectl apply -f test.yaml")
            .with_description("Test dependency");

        assert_eq!(dep.name, "Test");
        assert_eq!(dep.group, "example.com");
        assert!(dep.required);
        assert!(dep.install_hint.is_some());
    }

    #[test]
    fn test_optional_dependency() {
        let dep = Dependency::new("Test", "example.com", "v1", "TestResource").optional();
        assert!(!dep.required);
    }

    #[test]
    fn test_cache_key() {
        let key = DependencyChecker::cache_key("apps", "v1", "Deployment");
        assert_eq!(key, "apps/v1/Deployment");
    }
}
