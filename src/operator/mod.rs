//! Platform Operator for Kubernetes
//!
//! This module provides CRDs and controllers for:
//! - PlatformApp: Install applications via Helm and/or KCL manifests
//! - GitOpsApp: Manage FluxCD GitRepository and Kustomization resources
//! - CrossplaneResource: Manage Crossplane composite resources and claims
//! - GoogleWorkspace: User/group sync and RBAC from Google Workspace

pub mod connectors;
pub mod controllers;
pub mod crossplane;
pub mod dependencies;
pub mod flux;
pub mod google_workspace;
pub mod helm;
pub mod kcl;
pub mod types;
pub mod vault;
pub mod webhooks;

use thiserror::Error;

/// Errors that can occur during operator operations
#[derive(Error, Debug)]
pub enum OperatorError {
    #[error("Kubernetes API error: {0}")]
    KubeApi(#[from] kube::Error),

    #[error("Helm execution error: {0}")]
    Helm(String),

    #[error("KCL execution error: {0}")]
    Kcl(String),

    #[error("FluxCD error: {0}")]
    Flux(String),

    #[error("Crossplane error: {0}")]
    Crossplane(String),

    #[error("Google Workspace error: {0}")]
    GoogleWorkspace(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for operator operations
pub type Result<T> = std::result::Result<T, OperatorError>;

use std::sync::Arc;
use tokio::sync::RwLock;

/// Context shared across reconcilers
pub struct Context {
    /// Kubernetes client
    pub client: kube::Client,

    /// KCL executor for manifest generation
    pub kcl_executor: kcl::KclExecutor,

    /// Helm client for chart management
    pub helm_client: helm::HelmClient,

    /// FluxCD client for GitOps resources
    pub flux_client: flux::FluxClient,

    /// Crossplane client for composite resources
    pub crossplane_client: crossplane::CrossplaneClient,

    /// Google Workspace client (optional, initialized when GoogleWorkspaceConfig exists)
    pub google_workspace_client: Option<Arc<RwLock<google_workspace::GoogleWorkspaceClient>>>,

    /// Dependency checker for validating required operators/CRDs
    pub dependency_checker: Arc<dependencies::DependencyChecker>,

    /// Connector pool for external service connections (GCP, AWS, Azure, Vault)
    pub connector_pool: Arc<connectors::ConnectorPool>,
}

impl Context {
    /// Create a new context with default configuration
    pub async fn try_default() -> Result<Self> {
        let client = kube::Client::try_default().await?;

        // Initialize dependency checker
        let dependency_checker = Arc::new(dependencies::DependencyChecker::new(client.clone()));

        // Initialize connector pool (can be configured later)
        let connector_pool = Arc::new(connectors::ConnectorPool::new());

        Ok(Self {
            flux_client: flux::FluxClient::new(client.clone()),
            crossplane_client: crossplane::CrossplaneClient::new(client.clone()),
            dependency_checker,
            connector_pool,
            client,
            kcl_executor: kcl::KclExecutor::new(),
            helm_client: helm::HelmClient::new(),
            google_workspace_client: None, // Initialized by GoogleWorkspaceConfig controller
        })
    }

    /// Create context with custom connector pool
    pub async fn with_connectors(connector_pool: connectors::ConnectorPool) -> Result<Self> {
        let client = kube::Client::try_default().await?;
        let dependency_checker = Arc::new(dependencies::DependencyChecker::new(client.clone()));

        Ok(Self {
            flux_client: flux::FluxClient::new(client.clone()),
            crossplane_client: crossplane::CrossplaneClient::new(client.clone()),
            dependency_checker,
            connector_pool: Arc::new(connector_pool),
            client,
            kcl_executor: kcl::KclExecutor::new(),
            helm_client: helm::HelmClient::new(),
            google_workspace_client: None,
        })
    }

    /// Set the Google Workspace client
    pub fn set_google_workspace_client(
        &mut self,
        client: google_workspace::GoogleWorkspaceClient,
    ) {
        self.google_workspace_client = Some(Arc::new(RwLock::new(client)));
    }

    /// Check if all dependencies for a controller are available
    pub async fn check_dependencies(
        &self,
        deps: &[dependencies::Dependency],
    ) -> Vec<dependencies::DependencyCheckResult> {
        self.dependency_checker.check_all(deps).await
    }

    /// Get missing required dependencies
    pub async fn get_missing_dependencies(
        &self,
        deps: &[dependencies::Dependency],
    ) -> Vec<dependencies::DependencyCheckResult> {
        self.dependency_checker.get_missing(deps).await
    }
}
