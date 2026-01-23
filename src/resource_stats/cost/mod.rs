//! Cost calculation module

pub mod cloud;
pub mod static_pricing;

use std::any::Any;

use async_trait::async_trait;
use rust_decimal::Decimal;

use crate::resource_stats::types::resource_stats::{CostSummary, GpuResourceStats, ResourceSnapshot};

/// Error type for cost calculations
#[derive(thiserror::Error, Debug)]
pub enum CostError {
    #[error("Pricing not configured: {0}")]
    NotConfigured(String),

    #[error("Cloud API error: {0}")]
    CloudApi(String),

    #[error("Invalid pricing data: {0}")]
    InvalidPricing(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

/// Pricing rates for resources
#[derive(Clone, Debug)]
pub struct PricingRates {
    /// Cost per CPU core per hour
    pub cpu_per_core_hour: Decimal,
    /// Cost per GiB memory per hour
    pub memory_per_gib_hour: Decimal,
    /// GPU pricing: (model_pattern, rate per hour)
    pub gpu_pricing: Vec<(String, Decimal)>,
    /// Currency code (USD, EUR, etc.)
    pub currency: String,
}

impl Default for PricingRates {
    fn default() -> Self {
        Self {
            // Default to approximate GCP pricing
            cpu_per_core_hour: Decimal::new(31611, 6), // $0.031611
            memory_per_gib_hour: Decimal::new(4237, 6), // $0.004237
            gpu_pricing: vec![
                (".*A100.*".to_string(), Decimal::new(293, 2)), // $2.93
                (".*V100.*".to_string(), Decimal::new(248, 2)), // $2.48
                (".*T4.*".to_string(), Decimal::new(35, 2)),    // $0.35
            ],
            currency: "USD".to_string(),
        }
    }
}

/// Trait for cost calculation
#[async_trait]
pub trait CostCalculator: Send + Sync {
    /// Get current pricing rates (may fetch from API)
    async fn get_rates(&self, node_type: Option<&str>) -> Result<PricingRates, CostError>;

    /// Calculate cost from resource snapshot
    fn calculate_cost(&self, snapshot: &ResourceSnapshot, rates: &PricingRates) -> CostSummary;

    /// Calculate GPU cost
    fn calculate_gpu_cost(
        &self,
        gpu_stats: &[GpuResourceStats],
        rates: &PricingRates,
    ) -> Option<Decimal>;

    /// Get as Any for downcasting
    fn as_any(&self) -> &dyn Any;
}

/// Trait for cloud pricing providers
#[async_trait]
pub trait CloudPricingProvider: Send + Sync {
    /// Fetch current pricing for a region
    async fn fetch_pricing(&self, region: &str) -> Result<PricingRates, CostError>;

    /// Check if credentials are valid
    async fn validate_credentials(&self) -> Result<(), CostError>;
}

/// Calculate projected monthly cost from hourly rate
pub fn project_monthly(hourly: Decimal) -> Decimal {
    // Average hours per month: 730 (365.25 * 24 / 12)
    hourly * Decimal::from(730)
}

/// Format decimal as currency string
pub fn format_currency(amount: Decimal, currency: &str) -> String {
    match currency {
        "USD" => format!("${:.2}", amount),
        "EUR" => format!("{:.2}", amount),
        "GBP" => format!("{:.2}", amount),
        _ => format!("{:.2} {}", amount, currency),
    }
}
