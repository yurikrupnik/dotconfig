pub mod azure;

#[cfg(feature = "gcp")]
pub mod gcp;

#[cfg(feature = "aws")]
pub mod aws;

use crate::{DatabasePricing, RegistryPricing, StoragePricing};

/// Provider pricing data
pub struct ProviderPricing {
    pub registry: RegistryPricing,
    pub database: DatabasePricing,
    pub storage: StoragePricing,
}

/// Trait for pricing providers
pub trait PricingProvider {
    fn name(&self) -> &'static str;
}
