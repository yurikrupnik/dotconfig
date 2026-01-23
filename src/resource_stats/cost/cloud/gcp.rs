//! GCP Cloud Billing API integration
//!
//! Uses the Cloud Billing API to fetch current compute pricing.
//! Requires a service account with `roles/billing.viewer` permission.

use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;

use super::{CloudPricingProvider, InstancePricing};
use crate::resource_stats::cost::{CostError, PricingRates};

/// GCP pricing provider using Cloud Billing API
pub struct GcpPricingProvider {
    #[allow(dead_code)]
    project_id: String,
    credentials: Option<String>,
    /// Cached instance type mappings
    instance_specs: HashMap<String, InstanceSpec>,
}

/// Specification for a GCP instance type
#[derive(Debug, Clone)]
struct InstanceSpec {
    vcpus: i32,
    memory_gib: f64,
    gpu_count: Option<i32>,
    gpu_type: Option<String>,
}

impl GcpPricingProvider {
    pub fn new(project_id: String) -> Self {
        Self {
            project_id,
            credentials: None,
            instance_specs: Self::build_instance_specs(),
        }
    }

    pub fn with_credentials(mut self, credentials: String) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Build a map of known GCP instance types and their specs
    fn build_instance_specs() -> HashMap<String, InstanceSpec> {
        let mut specs = HashMap::new();

        // N2 standard instances
        for n in [2, 4, 8, 16, 32, 48, 64, 80, 96, 128] {
            specs.insert(
                format!("n2-standard-{}", n),
                InstanceSpec {
                    vcpus: n,
                    memory_gib: n as f64 * 4.0,
                    gpu_count: None,
                    gpu_type: None,
                },
            );
        }

        // N2 highmem instances
        for n in [2, 4, 8, 16, 32, 48, 64, 80, 96, 128] {
            specs.insert(
                format!("n2-highmem-{}", n),
                InstanceSpec {
                    vcpus: n,
                    memory_gib: n as f64 * 8.0,
                    gpu_count: None,
                    gpu_type: None,
                },
            );
        }

        // E2 instances (cost-optimized)
        for n in [2, 4, 8, 16, 32] {
            specs.insert(
                format!("e2-standard-{}", n),
                InstanceSpec {
                    vcpus: n,
                    memory_gib: n as f64 * 4.0,
                    gpu_count: None,
                    gpu_type: None,
                },
            );
        }

        // GPU instances (A2 with A100)
        specs.insert(
            "a2-highgpu-1g".to_string(),
            InstanceSpec {
                vcpus: 12,
                memory_gib: 85.0,
                gpu_count: Some(1),
                gpu_type: Some("nvidia-tesla-a100".to_string()),
            },
        );
        specs.insert(
            "a2-highgpu-2g".to_string(),
            InstanceSpec {
                vcpus: 24,
                memory_gib: 170.0,
                gpu_count: Some(2),
                gpu_type: Some("nvidia-tesla-a100".to_string()),
            },
        );
        specs.insert(
            "a2-highgpu-4g".to_string(),
            InstanceSpec {
                vcpus: 48,
                memory_gib: 340.0,
                gpu_count: Some(4),
                gpu_type: Some("nvidia-tesla-a100".to_string()),
            },
        );
        specs.insert(
            "a2-highgpu-8g".to_string(),
            InstanceSpec {
                vcpus: 96,
                memory_gib: 680.0,
                gpu_count: Some(8),
                gpu_type: Some("nvidia-tesla-a100".to_string()),
            },
        );

        // G2 instances (L4 GPU)
        specs.insert(
            "g2-standard-4".to_string(),
            InstanceSpec {
                vcpus: 4,
                memory_gib: 16.0,
                gpu_count: Some(1),
                gpu_type: Some("nvidia-l4".to_string()),
            },
        );
        specs.insert(
            "g2-standard-8".to_string(),
            InstanceSpec {
                vcpus: 8,
                memory_gib: 32.0,
                gpu_count: Some(1),
                gpu_type: Some("nvidia-l4".to_string()),
            },
        );

        specs
    }

    /// Get base pricing rates for a region (approximate, for when API is unavailable)
    fn get_fallback_rates(&self, region: &str) -> PricingRates {
        // Approximate rates as of 2024 for us-central1
        // Different regions have slightly different pricing
        let region_multiplier = match region {
            r if r.starts_with("us-") => Decimal::from(1),
            r if r.starts_with("europe-") => Decimal::new(112, 2), // ~12% more
            r if r.starts_with("asia-") => Decimal::new(115, 2),   // ~15% more
            _ => Decimal::from(1),
        };

        let base_cpu = Decimal::new(31611, 6); // $0.031611 per vCPU hour
        let base_mem = Decimal::new(4237, 6);  // $0.004237 per GB hour

        PricingRates {
            cpu_per_core_hour: base_cpu * region_multiplier,
            memory_per_gib_hour: base_mem * region_multiplier,
            gpu_pricing: vec![
                (".*A100.*".to_string(), Decimal::new(293, 2)),  // $2.93/hr
                (".*L4.*".to_string(), Decimal::new(72, 2)),     // $0.72/hr
                (".*T4.*".to_string(), Decimal::new(35, 2)),     // $0.35/hr
                (".*V100.*".to_string(), Decimal::new(248, 2)),  // $2.48/hr
                (".*P100.*".to_string(), Decimal::new(146, 2)),  // $1.46/hr
            ],
            currency: "USD".to_string(),
        }
    }

    /// Fetch pricing from GCP Cloud Billing API
    async fn fetch_from_api(&self, _region: &str) -> Result<PricingRates, CostError> {
        // TODO: Implement actual API call
        // This would use the Cloud Billing API:
        // https://cloud.google.com/billing/v1/how-tos/catalog-api
        //
        // Endpoint: GET https://cloudbilling.googleapis.com/v1/services/{service}/skus
        // Service ID for Compute Engine: 6F81-5844-456A
        //
        // For now, return an error to trigger fallback
        Err(CostError::CloudApi(
            "GCP Cloud Billing API not yet implemented. Using fallback pricing.".into(),
        ))
    }
}

#[async_trait]
impl CloudPricingProvider for GcpPricingProvider {
    fn name(&self) -> &'static str {
        "gcp"
    }

    async fn fetch_pricing(&self, region: &str) -> Result<PricingRates, CostError> {
        // Try API first, fall back to static rates
        match self.fetch_from_api(region).await {
            Ok(rates) => Ok(rates),
            Err(_) => {
                tracing::info!("Using fallback GCP pricing for region {}", region);
                Ok(self.get_fallback_rates(region))
            }
        }
    }

    async fn validate_credentials(&self) -> Result<(), CostError> {
        if self.credentials.is_none() {
            // Try to use application default credentials
            if std::env::var("GOOGLE_APPLICATION_CREDENTIALS").is_err() {
                return Err(CostError::NotConfigured(
                    "GCP credentials not configured. Set GOOGLE_APPLICATION_CREDENTIALS or provide service account JSON.".into()
                ));
            }
        }
        Ok(())
    }

    async fn get_instance_pricing(
        &self,
        instance_type: &str,
        region: &str,
    ) -> Result<InstancePricing, CostError> {
        let rates = self.fetch_pricing(region).await?;

        let spec = self.instance_specs.get(instance_type).ok_or_else(|| {
            CostError::NotConfigured(format!("Unknown instance type: {}", instance_type))
        })?;

        // Calculate hourly cost
        let cpu_cost = rates.cpu_per_core_hour * Decimal::from(spec.vcpus);
        let mem_cost = rates.memory_per_gib_hour * Decimal::from_str(&spec.memory_gib.to_string())
            .unwrap_or(Decimal::ZERO);

        let gpu_cost = if let (Some(count), Some(gpu_type)) = (&spec.gpu_count, &spec.gpu_type) {
            rates
                .gpu_pricing
                .iter()
                .find(|(pattern, _)| {
                    regex::Regex::new(pattern)
                        .map(|re| re.is_match(gpu_type))
                        .unwrap_or(false)
                })
                .map(|(_, rate)| *rate * Decimal::from(*count))
                .unwrap_or(Decimal::ZERO)
        } else {
            Decimal::ZERO
        };

        let hourly_cost = cpu_cost + mem_cost + gpu_cost;

        Ok(InstancePricing {
            instance_type: instance_type.to_string(),
            region: region.to_string(),
            vcpus: spec.vcpus,
            memory_gib: spec.memory_gib,
            hourly_cost,
            gpu_count: spec.gpu_count,
            gpu_type: spec.gpu_type.clone(),
            spot_hourly_cost: Some(hourly_cost * Decimal::new(30, 2)), // ~70% discount for spot
        })
    }
}
