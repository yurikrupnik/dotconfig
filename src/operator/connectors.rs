//! Connector Pool for Platform Operator
//!
//! Manages authenticated connections to external services with:
//! - Token refresh and rotation
//! - Rate limiting with exponential backoff
//! - Connection pooling and reuse
//! - Health checks

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::operator::OperatorError;

/// Rate limiter configuration
#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    /// Maximum requests per second
    pub max_rps: u32,
    /// Maximum concurrent requests
    pub max_concurrent: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_rps: 10,
            max_concurrent: 5,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(60),
            backoff_multiplier: 2.0,
        }
    }
}

/// Rate limiter with token bucket algorithm
pub struct RateLimiter {
    config: RateLimitConfig,
    /// Semaphore for concurrency control
    semaphore: Arc<Semaphore>,
    /// Last request time for rate limiting
    last_request: Arc<RwLock<Instant>>,
    /// Current backoff duration (increases on failures)
    current_backoff: Arc<RwLock<Duration>>,
    /// Request counter for metrics
    request_count: AtomicU64,
    /// Error counter for metrics
    error_count: AtomicU64,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(config.max_concurrent as usize)),
            last_request: Arc::new(RwLock::new(Instant::now())),
            current_backoff: Arc::new(RwLock::new(config.initial_backoff)),
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            config,
        }
    }

    /// Execute a function with rate limiting and backoff
    pub async fn execute<F, Fut, T, E>(&self, f: F) -> Result<T, E>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Debug,
    {
        // Acquire semaphore permit for concurrency control
        let _permit = self.semaphore.acquire().await.expect("Semaphore closed");

        // Rate limiting: ensure minimum time between requests
        let min_interval = Duration::from_secs_f64(1.0 / self.config.max_rps as f64);
        {
            let mut last = self.last_request.write().await;
            let elapsed = last.elapsed();
            if elapsed < min_interval {
                sleep(min_interval - elapsed).await;
            }
            *last = Instant::now();
        }

        self.request_count.fetch_add(1, Ordering::Relaxed);

        match f().await {
            Ok(result) => {
                // Reset backoff on success
                let mut backoff = self.current_backoff.write().await;
                *backoff = self.config.initial_backoff;
                Ok(result)
            }
            Err(e) => {
                self.error_count.fetch_add(1, Ordering::Relaxed);

                // Increase backoff on failure
                let mut backoff = self.current_backoff.write().await;
                *backoff = Duration::from_secs_f64(
                    (backoff.as_secs_f64() * self.config.backoff_multiplier)
                        .min(self.config.max_backoff.as_secs_f64()),
                );
                debug!("Request failed, backoff increased to {:?}", *backoff);

                Err(e)
            }
        }
    }

    /// Execute with retry and exponential backoff
    pub async fn execute_with_retry<F, Fut, T, E>(
        &self,
        f: F,
        max_retries: u32,
    ) -> Result<T, E>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Debug,
    {
        let mut attempts = 0;
        let mut last_error: Option<E> = None;

        while attempts <= max_retries {
            match self.execute(&f).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    attempts += 1;
                    if attempts <= max_retries {
                        let backoff = *self.current_backoff.read().await;
                        warn!(
                            "Request failed (attempt {}/{}), retrying in {:?}: {:?}",
                            attempts,
                            max_retries + 1,
                            backoff,
                            e
                        );
                        sleep(backoff).await;
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.expect("Should have error after retries"))
    }

    /// Get metrics
    pub fn metrics(&self) -> (u64, u64) {
        (
            self.request_count.load(Ordering::Relaxed),
            self.error_count.load(Ordering::Relaxed),
        )
    }
}

/// Token/credential state
#[derive(Clone, Debug)]
pub enum CredentialState {
    /// Valid credentials with expiration
    Valid {
        expires_at: Option<Instant>,
    },
    /// Credentials need refresh
    NeedsRefresh,
    /// Credentials are invalid/missing
    Invalid(String),
}

/// Base trait for connectors
#[async_trait::async_trait]
pub trait Connector: Send + Sync {
    /// Get the connector name
    fn name(&self) -> &str;

    /// Check if the connector is healthy
    async fn health_check(&self) -> Result<(), OperatorError>;

    /// Get the credential state
    async fn credential_state(&self) -> CredentialState;

    /// Refresh credentials if needed
    async fn refresh_credentials(&self) -> Result<(), OperatorError>;
}

/// GCP Connector for Google Cloud APIs
pub struct GcpConnector {
    name: String,
    project_id: Option<String>,
    rate_limiter: RateLimiter,
    credentials: Arc<RwLock<Option<GcpCredentials>>>,
}

#[derive(Clone)]
struct GcpCredentials {
    access_token: String,
    expires_at: Instant,
}

impl GcpConnector {
    pub fn new(project_id: Option<String>) -> Self {
        Self {
            name: "gcp".to_string(),
            project_id,
            rate_limiter: RateLimiter::new(RateLimitConfig {
                max_rps: 100,     // GCP allows higher rates
                max_concurrent: 10,
                ..Default::default()
            }),
            credentials: Arc::new(RwLock::new(None)),
        }
    }

    pub fn project_id(&self) -> Option<&str> {
        self.project_id.as_deref()
    }

    /// Execute a GCP API call with rate limiting
    pub async fn call<F, Fut, T>(&self, f: F) -> Result<T, OperatorError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, OperatorError>>,
    {
        // Ensure credentials are valid
        if matches!(self.credential_state().await, CredentialState::NeedsRefresh | CredentialState::Invalid(_)) {
            self.refresh_credentials().await?;
        }

        self.rate_limiter
            .execute_with_retry(f, 3)
            .await
    }
}

#[async_trait::async_trait]
impl Connector for GcpConnector {
    fn name(&self) -> &str {
        &self.name
    }

    async fn health_check(&self) -> Result<(), OperatorError> {
        // Try to get credentials
        self.refresh_credentials().await
    }

    async fn credential_state(&self) -> CredentialState {
        let creds = self.credentials.read().await;
        match creds.as_ref() {
            Some(c) => {
                if c.expires_at > Instant::now() + Duration::from_secs(300) {
                    CredentialState::Valid {
                        expires_at: Some(c.expires_at),
                    }
                } else {
                    CredentialState::NeedsRefresh
                }
            }
            None => CredentialState::NeedsRefresh,
        }
    }

    async fn refresh_credentials(&self) -> Result<(), OperatorError> {
        // In a real implementation, this would use google-cloud-auth or similar
        // For now, we'll try to get credentials from the environment/metadata server
        debug!("Refreshing GCP credentials");

        // Placeholder: In production, use gcp_auth crate or workload identity
        // let auth = gcp_auth::AuthenticationManager::new().await?;
        // let token = auth.get_token(&["https://www.googleapis.com/auth/cloud-platform"]).await?;

        // For now, just mark as valid (actual implementation would get real token)
        let mut creds = self.credentials.write().await;
        *creds = Some(GcpCredentials {
            access_token: "placeholder".to_string(),
            expires_at: Instant::now() + Duration::from_secs(3600),
        });

        info!("GCP credentials refreshed");
        Ok(())
    }
}

/// AWS Connector for AWS APIs
pub struct AwsConnector {
    name: String,
    region: String,
    rate_limiter: RateLimiter,
    credentials: Arc<RwLock<Option<AwsCredentials>>>,
}

#[derive(Clone)]
struct AwsCredentials {
    access_key_id: String,
    secret_access_key: String,
    session_token: Option<String>,
    expires_at: Option<Instant>,
}

impl AwsConnector {
    pub fn new(region: &str) -> Self {
        Self {
            name: "aws".to_string(),
            region: region.to_string(),
            rate_limiter: RateLimiter::new(RateLimitConfig {
                max_rps: 50,
                max_concurrent: 10,
                ..Default::default()
            }),
            credentials: Arc::new(RwLock::new(None)),
        }
    }

    pub fn region(&self) -> &str {
        &self.region
    }

    /// Execute an AWS API call with rate limiting
    pub async fn call<F, Fut, T>(&self, f: F) -> Result<T, OperatorError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, OperatorError>>,
    {
        if matches!(self.credential_state().await, CredentialState::NeedsRefresh | CredentialState::Invalid(_)) {
            self.refresh_credentials().await?;
        }

        self.rate_limiter
            .execute_with_retry(f, 3)
            .await
    }
}

#[async_trait::async_trait]
impl Connector for AwsConnector {
    fn name(&self) -> &str {
        &self.name
    }

    async fn health_check(&self) -> Result<(), OperatorError> {
        self.refresh_credentials().await
    }

    async fn credential_state(&self) -> CredentialState {
        let creds = self.credentials.read().await;
        match creds.as_ref() {
            Some(c) => match c.expires_at {
                Some(exp) if exp > Instant::now() + Duration::from_secs(300) => {
                    CredentialState::Valid {
                        expires_at: Some(exp),
                    }
                }
                Some(_) => CredentialState::NeedsRefresh,
                None => CredentialState::Valid { expires_at: None },
            },
            None => CredentialState::NeedsRefresh,
        }
    }

    async fn refresh_credentials(&self) -> Result<(), OperatorError> {
        debug!("Refreshing AWS credentials");

        // Placeholder: In production, use IRSA, IMDS, or environment variables
        // let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        // let creds = config.credentials_provider().provide_credentials().await?;

        let mut creds = self.credentials.write().await;
        *creds = Some(AwsCredentials {
            access_key_id: "placeholder".to_string(),
            secret_access_key: "placeholder".to_string(),
            session_token: None,
            expires_at: Some(Instant::now() + Duration::from_secs(3600)),
        });

        info!("AWS credentials refreshed");
        Ok(())
    }
}

/// Azure Connector
pub struct AzureConnector {
    name: String,
    tenant_id: String,
    subscription_id: Option<String>,
    rate_limiter: RateLimiter,
    credentials: Arc<RwLock<Option<AzureCredentials>>>,
}

#[derive(Clone)]
struct AzureCredentials {
    access_token: String,
    expires_at: Instant,
}

impl AzureConnector {
    pub fn new(tenant_id: &str, subscription_id: Option<String>) -> Self {
        Self {
            name: "azure".to_string(),
            tenant_id: tenant_id.to_string(),
            subscription_id,
            rate_limiter: RateLimiter::new(RateLimitConfig {
                max_rps: 50,
                max_concurrent: 10,
                ..Default::default()
            }),
            credentials: Arc::new(RwLock::new(None)),
        }
    }

    /// Execute an Azure API call with rate limiting
    pub async fn call<F, Fut, T>(&self, f: F) -> Result<T, OperatorError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, OperatorError>>,
    {
        if matches!(self.credential_state().await, CredentialState::NeedsRefresh | CredentialState::Invalid(_)) {
            self.refresh_credentials().await?;
        }

        self.rate_limiter
            .execute_with_retry(f, 3)
            .await
    }
}

#[async_trait::async_trait]
impl Connector for AzureConnector {
    fn name(&self) -> &str {
        &self.name
    }

    async fn health_check(&self) -> Result<(), OperatorError> {
        self.refresh_credentials().await
    }

    async fn credential_state(&self) -> CredentialState {
        let creds = self.credentials.read().await;
        match creds.as_ref() {
            Some(c) if c.expires_at > Instant::now() + Duration::from_secs(300) => {
                CredentialState::Valid {
                    expires_at: Some(c.expires_at),
                }
            }
            Some(_) => CredentialState::NeedsRefresh,
            None => CredentialState::NeedsRefresh,
        }
    }

    async fn refresh_credentials(&self) -> Result<(), OperatorError> {
        debug!("Refreshing Azure credentials");

        // Placeholder: Use azure_identity crate in production
        let mut creds = self.credentials.write().await;
        *creds = Some(AzureCredentials {
            access_token: "placeholder".to_string(),
            expires_at: Instant::now() + Duration::from_secs(3600),
        });

        info!("Azure credentials refreshed");
        Ok(())
    }
}

/// Vault Connector for HashiCorp Vault
pub struct VaultConnector {
    name: String,
    address: String,
    rate_limiter: RateLimiter,
    token: Arc<RwLock<Option<VaultToken>>>,
}

#[derive(Clone)]
struct VaultToken {
    token: String,
    expires_at: Option<Instant>,
    renewable: bool,
}

impl VaultConnector {
    pub fn new(address: &str) -> Self {
        Self {
            name: "vault".to_string(),
            address: address.to_string(),
            rate_limiter: RateLimiter::new(RateLimitConfig::default()),
            token: Arc::new(RwLock::new(None)),
        }
    }

    pub fn address(&self) -> &str {
        &self.address
    }

    /// Execute a Vault API call with rate limiting
    pub async fn call<F, Fut, T>(&self, f: F) -> Result<T, OperatorError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, OperatorError>>,
    {
        if matches!(self.credential_state().await, CredentialState::NeedsRefresh | CredentialState::Invalid(_)) {
            self.refresh_credentials().await?;
        }

        self.rate_limiter
            .execute_with_retry(f, 3)
            .await
    }
}

#[async_trait::async_trait]
impl Connector for VaultConnector {
    fn name(&self) -> &str {
        &self.name
    }

    async fn health_check(&self) -> Result<(), OperatorError> {
        // Check vault health endpoint
        // In production: GET {address}/v1/sys/health
        Ok(())
    }

    async fn credential_state(&self) -> CredentialState {
        let token = self.token.read().await;
        match token.as_ref() {
            Some(t) => match t.expires_at {
                Some(exp) if exp > Instant::now() + Duration::from_secs(300) => {
                    CredentialState::Valid {
                        expires_at: Some(exp),
                    }
                }
                Some(_) if t.renewable => CredentialState::NeedsRefresh,
                Some(_) => CredentialState::Invalid("Token expired and not renewable".to_string()),
                None => CredentialState::Valid { expires_at: None },
            },
            None => CredentialState::NeedsRefresh,
        }
    }

    async fn refresh_credentials(&self) -> Result<(), OperatorError> {
        debug!("Refreshing Vault token");

        // Placeholder: Use Kubernetes auth or token renewal in production
        let mut token = self.token.write().await;
        *token = Some(VaultToken {
            token: "placeholder".to_string(),
            expires_at: Some(Instant::now() + Duration::from_secs(3600)),
            renewable: true,
        });

        info!("Vault token refreshed");
        Ok(())
    }
}

/// Connector pool managing all connectors
pub struct ConnectorPool {
    /// GCP connector (optional)
    pub gcp: Option<Arc<GcpConnector>>,
    /// AWS connector (optional)
    pub aws: Option<Arc<AwsConnector>>,
    /// Azure connector (optional)
    pub azure: Option<Arc<AzureConnector>>,
    /// Vault connector (optional)
    pub vault: Option<Arc<VaultConnector>>,
    /// Custom connectors by name
    custom: Arc<RwLock<HashMap<String, Arc<dyn Connector>>>>,
}

impl ConnectorPool {
    /// Create an empty connector pool
    pub fn new() -> Self {
        Self {
            gcp: None,
            aws: None,
            azure: None,
            vault: None,
            custom: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create connector pool with GCP
    pub fn with_gcp(mut self, project_id: Option<String>) -> Self {
        self.gcp = Some(Arc::new(GcpConnector::new(project_id)));
        self
    }

    /// Create connector pool with AWS
    pub fn with_aws(mut self, region: &str) -> Self {
        self.aws = Some(Arc::new(AwsConnector::new(region)));
        self
    }

    /// Create connector pool with Azure
    pub fn with_azure(mut self, tenant_id: &str, subscription_id: Option<String>) -> Self {
        self.azure = Some(Arc::new(AzureConnector::new(tenant_id, subscription_id)));
        self
    }

    /// Create connector pool with Vault
    pub fn with_vault(mut self, address: &str) -> Self {
        self.vault = Some(Arc::new(VaultConnector::new(address)));
        self
    }

    /// Add a custom connector
    pub async fn add_connector(&self, name: &str, connector: Arc<dyn Connector>) {
        let mut custom = self.custom.write().await;
        custom.insert(name.to_string(), connector);
    }

    /// Get a custom connector by name
    pub async fn get_connector(&self, name: &str) -> Option<Arc<dyn Connector>> {
        let custom = self.custom.read().await;
        custom.get(name).cloned()
    }

    /// Health check all connectors
    pub async fn health_check_all(&self) -> HashMap<String, Result<(), String>> {
        let mut results = HashMap::new();

        if let Some(gcp) = &self.gcp {
            results.insert(
                "gcp".to_string(),
                gcp.health_check().await.map_err(|e| e.to_string()),
            );
        }

        if let Some(aws) = &self.aws {
            results.insert(
                "aws".to_string(),
                aws.health_check().await.map_err(|e| e.to_string()),
            );
        }

        if let Some(azure) = &self.azure {
            results.insert(
                "azure".to_string(),
                azure.health_check().await.map_err(|e| e.to_string()),
            );
        }

        if let Some(vault) = &self.vault {
            results.insert(
                "vault".to_string(),
                vault.health_check().await.map_err(|e| e.to_string()),
            );
        }

        let custom = self.custom.read().await;
        for (name, connector) in custom.iter() {
            results.insert(
                name.clone(),
                connector.health_check().await.map_err(|e| e.to_string()),
            );
        }

        results
    }

    /// Refresh all credentials that need refreshing
    pub async fn refresh_all_credentials(&self) {
        if let Some(gcp) = &self.gcp {
            if matches!(gcp.credential_state().await, CredentialState::NeedsRefresh) {
                if let Err(e) = gcp.refresh_credentials().await {
                    error!("Failed to refresh GCP credentials: {}", e);
                }
            }
        }

        if let Some(aws) = &self.aws {
            if matches!(aws.credential_state().await, CredentialState::NeedsRefresh) {
                if let Err(e) = aws.refresh_credentials().await {
                    error!("Failed to refresh AWS credentials: {}", e);
                }
            }
        }

        if let Some(azure) = &self.azure {
            if matches!(azure.credential_state().await, CredentialState::NeedsRefresh) {
                if let Err(e) = azure.refresh_credentials().await {
                    error!("Failed to refresh Azure credentials: {}", e);
                }
            }
        }

        if let Some(vault) = &self.vault {
            if matches!(vault.credential_state().await, CredentialState::NeedsRefresh) {
                if let Err(e) = vault.refresh_credentials().await {
                    error!("Failed to refresh Vault token: {}", e);
                }
            }
        }
    }

    /// Start background credential maintenance task
    pub fn start_maintenance_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes

            loop {
                interval.tick().await;
                debug!("Running credential maintenance");
                self.refresh_all_credentials().await;
            }
        })
    }
}

impl Default for ConnectorPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.max_rps, 10);
        assert_eq!(config.max_concurrent, 5);
    }

    #[test]
    fn test_connector_pool_builder() {
        let pool = ConnectorPool::new()
            .with_gcp(Some("my-project".to_string()))
            .with_aws("us-east-1");

        assert!(pool.gcp.is_some());
        assert!(pool.aws.is_some());
        assert!(pool.azure.is_none());
        assert!(pool.vault.is_none());
    }

    #[tokio::test]
    async fn test_gcp_connector_credential_state() {
        let connector = GcpConnector::new(Some("test-project".to_string()));
        let state = connector.credential_state().await;
        assert!(matches!(state, CredentialState::NeedsRefresh));
    }
}
