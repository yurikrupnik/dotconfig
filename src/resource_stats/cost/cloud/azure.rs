//! Azure Retail Prices API integration
//!
//! Uses the Azure Retail Prices API to fetch current VM pricing.
//! This API is public and doesn't require authentication for pricing queries.

use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;

use super::{CloudPricingProvider, InstancePricing};
use crate::resource_stats::cost::{CostError, PricingRates};

/// Azure pricing provider using Retail Prices API
pub struct AzurePricingProvider {
    subscription_id: Option<String>,
    /// Cached instance type mappings
    instance_specs: HashMap<String, InstanceSpec>,
}

/// Specification for an Azure VM size
#[derive(Debug, Clone)]
struct InstanceSpec {
    vcpus: i32,
    memory_gib: f64,
    gpu_count: Option<i32>,
    gpu_type: Option<String>,
    /// Base hourly price (East US)
    base_hourly_usd: Decimal,
}

impl AzurePricingProvider {
    pub fn new() -> Self {
        Self {
            subscription_id: None,
            instance_specs: Self::build_instance_specs(),
        }
    }

    pub fn with_subscription(mut self, subscription_id: String) -> Self {
        self.subscription_id = Some(subscription_id);
        self
    }

    /// Build a map of known Azure VM sizes and their specs
    fn build_instance_specs() -> HashMap<String, InstanceSpec> {
        let mut specs = HashMap::new();

        // D-series v5 (general purpose)
        let dv5_configs = [
            ("Standard_D2_v5", 2, 8.0, "0.096"),
            ("Standard_D4_v5", 4, 16.0, "0.192"),
            ("Standard_D8_v5", 8, 32.0, "0.384"),
            ("Standard_D16_v5", 16, 64.0, "0.768"),
            ("Standard_D32_v5", 32, 128.0, "1.536"),
            ("Standard_D48_v5", 48, 192.0, "2.304"),
            ("Standard_D64_v5", 64, 256.0, "3.072"),
            ("Standard_D96_v5", 96, 384.0, "4.608"),
        ];

        for (name, vcpus, memory, price) in dv5_configs {
            specs.insert(
                name.to_string(),
                InstanceSpec {
                    vcpus,
                    memory_gib: memory,
                    gpu_count: None,
                    gpu_type: None,
                    base_hourly_usd: Decimal::from_str(price).unwrap(),
                },
            );
        }

        // E-series v5 (memory optimized)
        let ev5_configs = [
            ("Standard_E2_v5", 2, 16.0, "0.126"),
            ("Standard_E4_v5", 4, 32.0, "0.252"),
            ("Standard_E8_v5", 8, 64.0, "0.504"),
            ("Standard_E16_v5", 16, 128.0, "1.008"),
            ("Standard_E32_v5", 32, 256.0, "2.016"),
            ("Standard_E48_v5", 48, 384.0, "3.024"),
            ("Standard_E64_v5", 64, 512.0, "4.032"),
            ("Standard_E96_v5", 96, 672.0, "6.048"),
        ];

        for (name, vcpus, memory, price) in ev5_configs {
            specs.insert(
                name.to_string(),
                InstanceSpec {
                    vcpus,
                    memory_gib: memory,
                    gpu_count: None,
                    gpu_type: None,
                    base_hourly_usd: Decimal::from_str(price).unwrap(),
                },
            );
        }

        // F-series v2 (compute optimized)
        let fv2_configs = [
            ("Standard_F2s_v2", 2, 4.0, "0.085"),
            ("Standard_F4s_v2", 4, 8.0, "0.170"),
            ("Standard_F8s_v2", 8, 16.0, "0.340"),
            ("Standard_F16s_v2", 16, 32.0, "0.680"),
            ("Standard_F32s_v2", 32, 64.0, "1.360"),
            ("Standard_F48s_v2", 48, 96.0, "2.040"),
            ("Standard_F64s_v2", 64, 128.0, "2.720"),
            ("Standard_F72s_v2", 72, 144.0, "3.060"),
        ];

        for (name, vcpus, memory, price) in fv2_configs {
            specs.insert(
                name.to_string(),
                InstanceSpec {
                    vcpus,
                    memory_gib: memory,
                    gpu_count: None,
                    gpu_type: None,
                    base_hourly_usd: Decimal::from_str(price).unwrap(),
                },
            );
        }

        // NC-series v3 (V100 GPU)
        specs.insert(
            "Standard_NC6s_v3".to_string(),
            InstanceSpec {
                vcpus: 6,
                memory_gib: 112.0,
                gpu_count: Some(1),
                gpu_type: Some("nvidia-tesla-v100".to_string()),
                base_hourly_usd: Decimal::from_str("3.06").unwrap(),
            },
        );
        specs.insert(
            "Standard_NC12s_v3".to_string(),
            InstanceSpec {
                vcpus: 12,
                memory_gib: 224.0,
                gpu_count: Some(2),
                gpu_type: Some("nvidia-tesla-v100".to_string()),
                base_hourly_usd: Decimal::from_str("6.12").unwrap(),
            },
        );
        specs.insert(
            "Standard_NC24s_v3".to_string(),
            InstanceSpec {
                vcpus: 24,
                memory_gib: 448.0,
                gpu_count: Some(4),
                gpu_type: Some("nvidia-tesla-v100".to_string()),
                base_hourly_usd: Decimal::from_str("12.24").unwrap(),
            },
        );

        // ND-series A100
        specs.insert(
            "Standard_ND96asr_v4".to_string(),
            InstanceSpec {
                vcpus: 96,
                memory_gib: 900.0,
                gpu_count: Some(8),
                gpu_type: Some("nvidia-a100".to_string()),
                base_hourly_usd: Decimal::from_str("27.20").unwrap(),
            },
        );

        // NC T4 series
        let nct4_configs = [
            ("Standard_NC4as_T4_v3", 4, 28.0, 1, "0.526"),
            ("Standard_NC8as_T4_v3", 8, 56.0, 1, "0.752"),
            ("Standard_NC16as_T4_v3", 16, 110.0, 1, "1.204"),
            ("Standard_NC64as_T4_v3", 64, 440.0, 4, "4.352"),
        ];

        for (name, vcpus, memory, gpus, price) in nct4_configs {
            specs.insert(
                name.to_string(),
                InstanceSpec {
                    vcpus,
                    memory_gib: memory,
                    gpu_count: Some(gpus),
                    gpu_type: Some("nvidia-tesla-t4".to_string()),
                    base_hourly_usd: Decimal::from_str(price).unwrap(),
                },
            );
        }

        specs
    }

    /// Get region pricing multiplier
    fn get_region_multiplier(region: &str) -> Decimal {
        match region {
            "eastus" | "eastus2" | "centralus" => Decimal::from(1),
            "westus" | "westus2" | "westus3" => Decimal::new(105, 2),
            r if r.starts_with("europe") || r.starts_with("west") || r.starts_with("north") => {
                Decimal::new(112, 2)
            }
            r if r.starts_with("asia") || r.starts_with("japan") || r.starts_with("korea") => {
                Decimal::new(118, 2)
            }
            r if r.starts_with("brazil") || r.starts_with("south") => Decimal::new(145, 2),
            _ => Decimal::from(1),
        }
    }

    /// Get fallback pricing rates
    fn get_fallback_rates(&self, region: &str) -> PricingRates {
        let multiplier = Self::get_region_multiplier(region);

        // Approximate per-vCPU and per-GB rates based on D-series
        let base_cpu = Decimal::new(24, 3); // ~$0.024 per vCPU hour
        let base_mem = Decimal::new(3, 3);  // ~$0.003 per GB hour

        PricingRates {
            cpu_per_core_hour: base_cpu * multiplier,
            memory_per_gib_hour: base_mem * multiplier,
            gpu_pricing: vec![
                (".*A100.*".to_string(), Decimal::new(340, 2)),  // ~$3.40/GPU/hr
                (".*V100.*".to_string(), Decimal::new(306, 2)),  // ~$3.06/GPU/hr
                (".*T4.*".to_string(), Decimal::new(53, 2)),     // ~$0.53/GPU/hr
                (".*A10.*".to_string(), Decimal::new(180, 2)),   // ~$1.80/GPU/hr
            ],
            currency: "USD".to_string(),
        }
    }

    /// Fetch pricing from Azure Retail Prices API
    async fn fetch_from_api(&self, _region: &str) -> Result<PricingRates, CostError> {
        // The Azure Retail Prices API is public:
        // https://prices.azure.com/api/retail/prices
        //
        // Example query:
        // https://prices.azure.com/api/retail/prices?$filter=serviceName eq 'Virtual Machines' and armRegionName eq 'eastus'
        //
        // TODO: Implement actual API call using reqwest
        //
        // For now, return an error to trigger fallback
        Err(CostError::CloudApi(
            "Azure Retail Prices API not yet implemented. Using fallback pricing.".into(),
        ))
    }
}

impl Default for AzurePricingProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CloudPricingProvider for AzurePricingProvider {
    fn name(&self) -> &'static str {
        "azure"
    }

    async fn fetch_pricing(&self, region: &str) -> Result<PricingRates, CostError> {
        // Try API first, fall back to static rates
        match self.fetch_from_api(region).await {
            Ok(rates) => Ok(rates),
            Err(_) => {
                tracing::info!("Using fallback Azure pricing for region {}", region);
                Ok(self.get_fallback_rates(region))
            }
        }
    }

    async fn validate_credentials(&self) -> Result<(), CostError> {
        // Azure Retail Prices API doesn't require authentication
        // For Cost Management API, we would need credentials
        Ok(())
    }

    async fn get_instance_pricing(
        &self,
        instance_type: &str,
        region: &str,
    ) -> Result<InstancePricing, CostError> {
        let spec = self.instance_specs.get(instance_type).ok_or_else(|| {
            CostError::NotConfigured(format!("Unknown instance type: {}", instance_type))
        })?;

        let multiplier = Self::get_region_multiplier(region);
        let hourly_cost = spec.base_hourly_usd * multiplier;

        Ok(InstancePricing {
            instance_type: instance_type.to_string(),
            region: region.to_string(),
            vcpus: spec.vcpus,
            memory_gib: spec.memory_gib,
            hourly_cost,
            gpu_count: spec.gpu_count,
            gpu_type: spec.gpu_type.clone(),
            spot_hourly_cost: Some(hourly_cost * Decimal::new(35, 2)), // ~65% discount for spot
        })
    }
}
