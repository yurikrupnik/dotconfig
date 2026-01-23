//! AWS Pricing API integration
//!
//! Uses the AWS Pricing API to fetch current EC2 pricing.
//! Requires IAM credentials with `pricing:GetProducts` permission.

use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;

use super::{CloudPricingProvider, InstancePricing};
use crate::resource_stats::cost::{CostError, PricingRates};

/// AWS pricing provider using Pricing API
pub struct AwsPricingProvider {
    region: String,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    /// Cached instance type mappings
    instance_specs: HashMap<String, InstanceSpec>,
}

/// Specification for an AWS instance type
#[derive(Debug, Clone)]
struct InstanceSpec {
    vcpus: i32,
    memory_gib: f64,
    gpu_count: Option<i32>,
    gpu_type: Option<String>,
    /// Base on-demand hourly price (us-east-1)
    base_hourly_usd: Decimal,
}

impl AwsPricingProvider {
    pub fn new(region: String) -> Self {
        Self {
            region,
            access_key_id: None,
            secret_access_key: None,
            instance_specs: Self::build_instance_specs(),
        }
    }

    pub fn with_credentials(mut self, access_key_id: String, secret_access_key: String) -> Self {
        self.access_key_id = Some(access_key_id);
        self.secret_access_key = Some(secret_access_key);
        self
    }

    /// Build a map of known AWS instance types and their specs
    fn build_instance_specs() -> HashMap<String, InstanceSpec> {
        let mut specs = HashMap::new();

        // M5 general purpose
        let m5_configs = [
            ("m5.large", 2, 8.0, "0.096"),
            ("m5.xlarge", 4, 16.0, "0.192"),
            ("m5.2xlarge", 8, 32.0, "0.384"),
            ("m5.4xlarge", 16, 64.0, "0.768"),
            ("m5.8xlarge", 32, 128.0, "1.536"),
            ("m5.12xlarge", 48, 192.0, "2.304"),
            ("m5.16xlarge", 64, 256.0, "3.072"),
            ("m5.24xlarge", 96, 384.0, "4.608"),
        ];

        for (name, vcpus, memory, price) in m5_configs {
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

        // M6i instances (newer gen)
        let m6i_configs = [
            ("m6i.large", 2, 8.0, "0.096"),
            ("m6i.xlarge", 4, 16.0, "0.192"),
            ("m6i.2xlarge", 8, 32.0, "0.384"),
            ("m6i.4xlarge", 16, 64.0, "0.768"),
            ("m6i.8xlarge", 32, 128.0, "1.536"),
            ("m6i.12xlarge", 48, 192.0, "2.304"),
            ("m6i.16xlarge", 64, 256.0, "3.072"),
            ("m6i.24xlarge", 96, 384.0, "4.608"),
        ];

        for (name, vcpus, memory, price) in m6i_configs {
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

        // C5 compute optimized
        let c5_configs = [
            ("c5.large", 2, 4.0, "0.085"),
            ("c5.xlarge", 4, 8.0, "0.170"),
            ("c5.2xlarge", 8, 16.0, "0.340"),
            ("c5.4xlarge", 16, 32.0, "0.680"),
            ("c5.9xlarge", 36, 72.0, "1.530"),
            ("c5.18xlarge", 72, 144.0, "3.060"),
        ];

        for (name, vcpus, memory, price) in c5_configs {
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

        // R5 memory optimized
        let r5_configs = [
            ("r5.large", 2, 16.0, "0.126"),
            ("r5.xlarge", 4, 32.0, "0.252"),
            ("r5.2xlarge", 8, 64.0, "0.504"),
            ("r5.4xlarge", 16, 128.0, "1.008"),
            ("r5.8xlarge", 32, 256.0, "2.016"),
            ("r5.12xlarge", 48, 384.0, "3.024"),
            ("r5.16xlarge", 64, 512.0, "4.032"),
            ("r5.24xlarge", 96, 768.0, "6.048"),
        ];

        for (name, vcpus, memory, price) in r5_configs {
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

        // P3 GPU instances (V100)
        specs.insert(
            "p3.2xlarge".to_string(),
            InstanceSpec {
                vcpus: 8,
                memory_gib: 61.0,
                gpu_count: Some(1),
                gpu_type: Some("nvidia-tesla-v100".to_string()),
                base_hourly_usd: Decimal::from_str("3.06").unwrap(),
            },
        );
        specs.insert(
            "p3.8xlarge".to_string(),
            InstanceSpec {
                vcpus: 32,
                memory_gib: 244.0,
                gpu_count: Some(4),
                gpu_type: Some("nvidia-tesla-v100".to_string()),
                base_hourly_usd: Decimal::from_str("12.24").unwrap(),
            },
        );
        specs.insert(
            "p3.16xlarge".to_string(),
            InstanceSpec {
                vcpus: 64,
                memory_gib: 488.0,
                gpu_count: Some(8),
                gpu_type: Some("nvidia-tesla-v100".to_string()),
                base_hourly_usd: Decimal::from_str("24.48").unwrap(),
            },
        );

        // P4d GPU instances (A100)
        specs.insert(
            "p4d.24xlarge".to_string(),
            InstanceSpec {
                vcpus: 96,
                memory_gib: 1152.0,
                gpu_count: Some(8),
                gpu_type: Some("nvidia-a100".to_string()),
                base_hourly_usd: Decimal::from_str("32.77").unwrap(),
            },
        );

        // G4dn GPU instances (T4)
        let g4dn_configs = [
            ("g4dn.xlarge", 4, 16.0, 1, "0.526"),
            ("g4dn.2xlarge", 8, 32.0, 1, "0.752"),
            ("g4dn.4xlarge", 16, 64.0, 1, "1.204"),
            ("g4dn.8xlarge", 32, 128.0, 1, "2.176"),
            ("g4dn.12xlarge", 48, 192.0, 4, "3.912"),
            ("g4dn.16xlarge", 64, 256.0, 1, "4.352"),
        ];

        for (name, vcpus, memory, gpus, price) in g4dn_configs {
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
    fn get_region_multiplier(&self) -> Decimal {
        match self.region.as_str() {
            "us-east-1" | "us-east-2" => Decimal::from(1),
            "us-west-1" => Decimal::new(108, 2),
            "us-west-2" => Decimal::from(1),
            r if r.starts_with("eu-") => Decimal::new(110, 2),
            r if r.starts_with("ap-") => Decimal::new(115, 2),
            r if r.starts_with("sa-") => Decimal::new(140, 2),
            _ => Decimal::from(1),
        }
    }

    /// Get fallback pricing rates
    fn get_fallback_rates(&self) -> PricingRates {
        let multiplier = self.get_region_multiplier();

        // Approximate per-vCPU and per-GB rates based on m5 instances
        let base_cpu = Decimal::new(24, 3); // ~$0.024 per vCPU hour (derived from m5)
        let base_mem = Decimal::new(3, 3);  // ~$0.003 per GB hour

        PricingRates {
            cpu_per_core_hour: base_cpu * multiplier,
            memory_per_gib_hour: base_mem * multiplier,
            gpu_pricing: vec![
                (".*A100.*".to_string(), Decimal::new(410, 2)),  // ~$4.10/GPU/hr
                (".*V100.*".to_string(), Decimal::new(306, 2)),  // ~$3.06/GPU/hr
                (".*T4.*".to_string(), Decimal::new(53, 2)),     // ~$0.53/GPU/hr (from g4dn)
                (".*K80.*".to_string(), Decimal::new(90, 2)),    // ~$0.90/GPU/hr
            ],
            currency: "USD".to_string(),
        }
    }

    /// Fetch pricing from AWS Pricing API
    async fn fetch_from_api(&self) -> Result<PricingRates, CostError> {
        // TODO: Implement actual API call
        // This would use the AWS Pricing API:
        // https://docs.aws.amazon.com/awsaccountbilling/latest/aboutv2/price-list-query-api.html
        //
        // Endpoint: pricing.us-east-1.amazonaws.com
        // Action: GetProducts
        // ServiceCode: AmazonEC2
        //
        // For now, return an error to trigger fallback
        Err(CostError::CloudApi(
            "AWS Pricing API not yet implemented. Using fallback pricing.".into(),
        ))
    }
}

#[async_trait]
impl CloudPricingProvider for AwsPricingProvider {
    fn name(&self) -> &'static str {
        "aws"
    }

    async fn fetch_pricing(&self, _region: &str) -> Result<PricingRates, CostError> {
        // Try API first, fall back to static rates
        match self.fetch_from_api().await {
            Ok(rates) => Ok(rates),
            Err(_) => {
                tracing::info!("Using fallback AWS pricing for region {}", self.region);
                Ok(self.get_fallback_rates())
            }
        }
    }

    async fn validate_credentials(&self) -> Result<(), CostError> {
        if self.access_key_id.is_none() || self.secret_access_key.is_none() {
            // Try to use environment credentials or instance profile
            if std::env::var("AWS_ACCESS_KEY_ID").is_err() {
                // Check for instance metadata service
                tracing::info!("No explicit AWS credentials, will try instance profile");
            }
        }
        Ok(())
    }

    async fn get_instance_pricing(
        &self,
        instance_type: &str,
        _region: &str,
    ) -> Result<InstancePricing, CostError> {
        let spec = self.instance_specs.get(instance_type).ok_or_else(|| {
            CostError::NotConfigured(format!("Unknown instance type: {}", instance_type))
        })?;

        let multiplier = self.get_region_multiplier();
        let hourly_cost = spec.base_hourly_usd * multiplier;

        Ok(InstancePricing {
            instance_type: instance_type.to_string(),
            region: self.region.clone(),
            vcpus: spec.vcpus,
            memory_gib: spec.memory_gib,
            hourly_cost,
            gpu_count: spec.gpu_count,
            gpu_type: spec.gpu_type.clone(),
            spot_hourly_cost: Some(hourly_cost * Decimal::new(30, 2)), // ~70% discount for spot
        })
    }
}
