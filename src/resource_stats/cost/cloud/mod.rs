//! Cloud pricing providers

pub mod aws;
pub mod azure;
pub mod gcp;

use async_trait::async_trait;

use super::{CostError, PricingRates};

/// Trait for cloud pricing providers
#[async_trait]
pub trait CloudPricingProvider: Send + Sync {
    /// Provider name
    fn name(&self) -> &'static str;

    /// Fetch current pricing for a region
    async fn fetch_pricing(&self, region: &str) -> Result<PricingRates, CostError>;

    /// Validate credentials
    async fn validate_credentials(&self) -> Result<(), CostError>;

    /// Get pricing for a specific instance type
    async fn get_instance_pricing(
        &self,
        instance_type: &str,
        region: &str,
    ) -> Result<InstancePricing, CostError>;
}

/// Pricing for a specific instance type
#[derive(Debug, Clone)]
pub struct InstancePricing {
    pub instance_type: String,
    pub region: String,
    pub vcpus: i32,
    pub memory_gib: f64,
    pub hourly_cost: rust_decimal::Decimal,
    pub gpu_count: Option<i32>,
    pub gpu_type: Option<String>,
    pub spot_hourly_cost: Option<rust_decimal::Decimal>,
}

/// Credentials for cloud providers
#[derive(Debug, Clone)]
pub enum CloudCredentials {
    /// GCP service account JSON
    Gcp {
        service_account_json: String,
        project_id: String,
    },
    /// AWS access keys
    Aws {
        access_key_id: String,
        secret_access_key: String,
        region: String,
    },
    /// Azure service principal
    Azure {
        tenant_id: String,
        client_id: String,
        client_secret: String,
        subscription_id: String,
    },
}
