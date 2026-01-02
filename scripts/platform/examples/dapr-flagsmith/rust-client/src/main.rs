//! Dapr + Flagsmith Feature Flags Client (Rust)
//!
//! A Rust client for evaluating feature flags using Dapr state store
//! and Flagsmith as the feature flag provider.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Feature flag value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FlagValue {
    Boolean(bool),
    String(String),
    Number(f64),
    Object(serde_json::Value),
}

/// Evaluation context for targeting
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvaluationContext {
    pub targeting_key: Option<String>,
    #[serde(flatten)]
    pub attributes: HashMap<String, String>,
}

/// Feature flag evaluation result
#[derive(Debug, Clone)]
pub struct EvaluationResult<T> {
    pub value: T,
    pub variant: Option<String>,
    pub reason: String,
}

/// Trait for feature flag providers
#[async_trait]
pub trait FeatureFlagProvider: Send + Sync {
    async fn get_boolean(&self, key: &str, default: bool, ctx: &EvaluationContext) -> Result<EvaluationResult<bool>>;
    async fn get_string(&self, key: &str, default: &str, ctx: &EvaluationContext) -> Result<EvaluationResult<String>>;
    async fn get_number(&self, key: &str, default: f64, ctx: &EvaluationContext) -> Result<EvaluationResult<f64>>;
    async fn refresh(&self) -> Result<()>;
}

/// Dapr client for state and pubsub
pub struct DaprClient {
    base_url: String,
    http: reqwest::Client,
}

impl DaprClient {
    pub fn new(dapr_port: u16) -> Self {
        Self {
            base_url: format!("http://localhost:{}", dapr_port),
            http: reqwest::Client::new(),
        }
    }

    /// Get state from Dapr state store
    pub async fn get_state<T: for<'de> Deserialize<'de>>(&self, store: &str, key: &str) -> Result<Option<T>> {
        let url = format!("{}/v1.0/state/{}/{}", self.base_url, store, key);
        let resp = self.http.get(&url).send().await?;

        if resp.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(None);
        }

        if !resp.status().is_success() {
            anyhow::bail!("Dapr state get failed: {}", resp.status());
        }

        let value = resp.json().await?;
        Ok(Some(value))
    }

    /// Save state to Dapr state store
    pub async fn save_state<T: Serialize>(&self, store: &str, key: &str, value: &T) -> Result<()> {
        let url = format!("{}/v1.0/state/{}", self.base_url, store);
        let payload = serde_json::json!([{
            "key": key,
            "value": value
        }]);

        let resp = self.http.post(&url).json(&payload).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!("Dapr state save failed: {}", resp.status());
        }

        Ok(())
    }

    /// Publish event to Dapr pub/sub
    pub async fn publish<T: Serialize>(&self, pubsub: &str, topic: &str, data: &T) -> Result<()> {
        let url = format!("{}/v1.0/publish/{}/{}", self.base_url, pubsub, topic);
        let resp = self.http.post(&url).json(data).send().await?;

        if !resp.status().is_success() {
            anyhow::bail!("Dapr publish failed: {}", resp.status());
        }

        Ok(())
    }
}

/// Flagsmith flag structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagsmithFlag {
    pub id: i64,
    pub feature: FlagsmithFeature,
    pub enabled: bool,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlagsmithFeature {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub flag_type: Option<String>,
}

/// Cached flags with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedFlags {
    flags: Vec<FlagsmithFlag>,
    timestamp: String,
}

/// Flagsmith provider with Dapr integration
pub struct FlagsmithProvider {
    api_key: String,
    api_url: String,
    dapr: DaprClient,
    state_store: String,
    cache_ttl_secs: u64,
    cache: Arc<RwLock<HashMap<String, FlagsmithFlag>>>,
    http: reqwest::Client,
}

impl FlagsmithProvider {
    pub fn new(
        api_key: String,
        api_url: Option<String>,
        dapr_port: u16,
        state_store: String,
        cache_ttl_secs: u64,
    ) -> Self {
        Self {
            api_key,
            api_url: api_url.unwrap_or_else(|| "https://edge.api.flagsmith.com/api/v1/".to_string()),
            dapr: DaprClient::new(dapr_port),
            state_store,
            cache_ttl_secs,
            cache: Arc::new(RwLock::new(HashMap::new())),
            http: reqwest::Client::new(),
        }
    }

    /// Initialize provider and load flags
    pub async fn initialize(&self) -> Result<()> {
        // Try loading from Dapr cache
        if let Some(cached) = self.dapr.get_state::<CachedFlags>(&self.state_store, "flagsmith-flags").await? {
            let age = chrono_age(&cached.timestamp);
            if age < self.cache_ttl_secs {
                let mut cache = self.cache.write().await;
                for flag in cached.flags {
                    cache.insert(flag.feature.name.clone(), flag);
                }
                println!("Loaded {} flags from Dapr cache", cache.len());
                return Ok(());
            }
        }

        // Fetch from Flagsmith
        self.refresh().await
    }

    /// Fetch flags from Flagsmith API
    async fn fetch_flags(&self) -> Result<Vec<FlagsmithFlag>> {
        let url = format!("{}flags/", self.api_url);
        let resp = self.http
            .get(&url)
            .header("X-Environment-Key", &self.api_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Flagsmith API error: {}", resp.status());
        }

        let flags: Vec<FlagsmithFlag> = resp.json().await?;
        Ok(flags)
    }

    /// Get flag by name
    async fn get_flag(&self, key: &str) -> Option<FlagsmithFlag> {
        let cache = self.cache.read().await;
        cache.get(key).cloned()
    }
}

#[async_trait]
impl FeatureFlagProvider for FlagsmithProvider {
    async fn get_boolean(&self, key: &str, default: bool, _ctx: &EvaluationContext) -> Result<EvaluationResult<bool>> {
        match self.get_flag(key).await {
            Some(flag) => Ok(EvaluationResult {
                value: flag.enabled,
                variant: Some(if flag.enabled { "on" } else { "off" }.to_string()),
                reason: "STATIC".to_string(),
            }),
            None => Ok(EvaluationResult {
                value: default,
                variant: None,
                reason: "DEFAULT".to_string(),
            }),
        }
    }

    async fn get_string(&self, key: &str, default: &str, _ctx: &EvaluationContext) -> Result<EvaluationResult<String>> {
        match self.get_flag(key).await {
            Some(flag) if flag.enabled => {
                let value = flag.value
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| default.to_string());
                Ok(EvaluationResult {
                    value,
                    variant: Some("on".to_string()),
                    reason: "STATIC".to_string(),
                })
            }
            _ => Ok(EvaluationResult {
                value: default.to_string(),
                variant: None,
                reason: "DEFAULT".to_string(),
            }),
        }
    }

    async fn get_number(&self, key: &str, default: f64, _ctx: &EvaluationContext) -> Result<EvaluationResult<f64>> {
        match self.get_flag(key).await {
            Some(flag) if flag.enabled => {
                let value = flag.value
                    .as_ref()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(default);
                Ok(EvaluationResult {
                    value,
                    variant: Some("on".to_string()),
                    reason: "STATIC".to_string(),
                })
            }
            _ => Ok(EvaluationResult {
                value: default,
                variant: None,
                reason: "DEFAULT".to_string(),
            }),
        }
    }

    async fn refresh(&self) -> Result<()> {
        let flags = self.fetch_flags().await?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.clear();
            for flag in &flags {
                cache.insert(flag.feature.name.clone(), flag.clone());
            }
        }

        // Save to Dapr
        let cached = CachedFlags {
            flags,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        self.dapr.save_state(&self.state_store, "flagsmith-flags", &cached).await?;

        println!("Refreshed flags from Flagsmith");
        Ok(())
    }
}

/// Calculate age in seconds from ISO timestamp
fn chrono_age(timestamp: &str) -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Parse ISO timestamp (simplified)
    if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(timestamp) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let then = ts.timestamp() as u64;
        now.saturating_sub(then)
    } else {
        u64::MAX // Force refresh on parse error
    }
}

// Add chrono for timestamp handling
mod chrono {
    pub use ::chrono::*;
}

#[tokio::main]
async fn main() -> Result<()> {
    // Configuration from environment
    let api_key = std::env::var("FLAGSMITH_API_KEY")
        .expect("FLAGSMITH_API_KEY required");
    let dapr_port = std::env::var("DAPR_HTTP_PORT")
        .unwrap_or_else(|_| "3500".to_string())
        .parse()
        .unwrap_or(3500);

    // Create provider
    let provider = FlagsmithProvider::new(
        api_key,
        None,
        dapr_port,
        "statestore".to_string(),
        60,
    );

    // Initialize
    provider.initialize().await?;

    // Example: Check feature flags
    let ctx = EvaluationContext::default();

    let new_dashboard = provider.get_boolean("new-dashboard", false, &ctx).await?;
    println!("new-dashboard: {} ({})", new_dashboard.value, new_dashboard.reason);

    let beta_features = provider.get_boolean("beta-features", false, &ctx).await?;
    println!("beta-features: {} ({})", beta_features.value, beta_features.reason);

    // With targeting
    let mut user_ctx = EvaluationContext::default();
    user_ctx.targeting_key = Some("premium-user".to_string());
    user_ctx.attributes.insert("plan".to_string(), "enterprise".to_string());

    let premium_flag = provider.get_boolean("premium-features", false, &user_ctx).await?;
    println!("premium-features (user): {} ({})", premium_flag.value, premium_flag.reason);

    Ok(())
}
