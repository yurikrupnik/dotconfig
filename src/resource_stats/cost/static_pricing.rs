//! Static pricing calculator from configuration

use std::any::Any;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use regex::Regex;
use rust_decimal::Decimal;
use tokio::sync::RwLock;

use super::{format_currency, project_monthly, CostCalculator, CostError, PricingRates};
use crate::resource_stats::types::cost_config::{CostConfig, StaticPricingSpec};
use crate::resource_stats::types::resource_stats::{CostSummary, GpuResourceStats, ResourceSnapshot};

/// Static pricing calculator using configured rates
pub struct StaticPricingCalculator {
    /// Default rates when no specific config matches
    default_rates: PricingRates,
    /// Cached rates from CostConfig CRDs
    config_rates: Arc<RwLock<Vec<ConfiguredRates>>>,
}

/// Rates configured from a CostConfig CRD
struct ConfiguredRates {
    /// Node selector labels to match
    node_selector: Option<std::collections::BTreeMap<String, String>>,
    /// Priority for matching (higher wins)
    priority: i32,
    /// The pricing rates
    rates: PricingRates,
}

impl StaticPricingCalculator {
    pub fn new() -> Self {
        Self {
            default_rates: PricingRates::default(),
            config_rates: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_default_rates(mut self, rates: PricingRates) -> Self {
        self.default_rates = rates;
        self
    }

    /// Update the default rates (used when cloud pricing is fetched)
    pub async fn update_rates(&self, rates: PricingRates) {
        let configured = ConfiguredRates {
            node_selector: None,
            priority: 0, // Base priority for cloud-fetched rates
            rates,
        };

        let mut cache = self.config_rates.write().await;
        // Remove existing default rates (no node selector)
        cache.retain(|c| c.node_selector.is_some());
        cache.push(configured);
        cache.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Update rates from a CostConfig CRD
    pub async fn update_from_config(&self, config: &CostConfig) -> Result<(), CostError> {
        let spec = &config.spec;

        let static_pricing = spec
            .static_pricing
            .as_ref()
            .ok_or_else(|| CostError::NotConfigured("No static pricing in config".into()))?;

        let rates = self.parse_static_pricing(static_pricing, &spec.currency)?;

        let configured = ConfiguredRates {
            node_selector: spec.node_selector.clone(),
            priority: spec.priority,
            rates,
        };

        let mut cache = self.config_rates.write().await;
        // Remove existing config with same selector
        cache.retain(|c| c.node_selector != configured.node_selector);
        cache.push(configured);
        // Sort by priority descending
        cache.sort_by(|a, b| b.priority.cmp(&a.priority));

        Ok(())
    }

    /// Parse static pricing spec into PricingRates
    fn parse_static_pricing(
        &self,
        spec: &StaticPricingSpec,
        currency: &str,
    ) -> Result<PricingRates, CostError> {
        let cpu_per_core_hour = Decimal::from_str(&spec.cpu_per_core_hour)
            .map_err(|e| CostError::Parse(format!("Invalid CPU pricing: {}", e)))?;

        let memory_per_gib_hour = Decimal::from_str(&spec.memory_per_gib_hour)
            .map_err(|e| CostError::Parse(format!("Invalid memory pricing: {}", e)))?;

        let gpu_pricing = spec
            .gpu_pricing
            .iter()
            .map(|g| {
                let rate = Decimal::from_str(&g.per_gpu_hour)
                    .map_err(|e| CostError::Parse(format!("Invalid GPU pricing: {}", e)))?;
                Ok((g.model_pattern.clone(), rate))
            })
            .collect::<Result<Vec<_>, CostError>>()?;

        Ok(PricingRates {
            cpu_per_core_hour,
            memory_per_gib_hour,
            gpu_pricing,
            currency: currency.to_string(),
        })
    }

    /// Find GPU pricing rate by model name
    fn find_gpu_rate(&self, model: &str, rates: &PricingRates) -> Option<Decimal> {
        for (pattern, rate) in &rates.gpu_pricing {
            if let Ok(re) = Regex::new(pattern) {
                if re.is_match(model) {
                    return Some(*rate);
                }
            }
        }
        None
    }
}

impl Default for StaticPricingCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CostCalculator for StaticPricingCalculator {
    async fn get_rates(&self, _node_type: Option<&str>) -> Result<PricingRates, CostError> {
        // For now, return default rates
        // TODO: Match node_type against configured node selectors
        let cache = self.config_rates.read().await;
        if let Some(first) = cache.first() {
            Ok(first.rates.clone())
        } else {
            Ok(self.default_rates.clone())
        }
    }

    fn calculate_cost(&self, snapshot: &ResourceSnapshot, rates: &PricingRates) -> CostSummary {
        // Convert millicores to cores
        let cpu_cores = Decimal::from(snapshot.cpu.usage_millicores) / Decimal::from(1000);
        let cpu_cost = cpu_cores * rates.cpu_per_core_hour;

        // Convert bytes to GiB
        let memory_gib =
            Decimal::from(snapshot.memory.usage_bytes) / Decimal::from(1024 * 1024 * 1024);
        let memory_cost = memory_gib * rates.memory_per_gib_hour;

        // GPU cost if present
        let gpu_cost = snapshot
            .gpu
            .as_ref()
            .map(|_| Decimal::ZERO); // Placeholder, would need GPU stats

        let total_cost = cpu_cost + memory_cost + gpu_cost.unwrap_or(Decimal::ZERO);

        // Calculate efficiency score
        let efficiency = if snapshot.cpu.requests_millicores > 0 {
            let actual = snapshot.cpu.usage_millicores as f64;
            let requested = snapshot.cpu.requests_millicores as f64;
            Some(actual / requested)
        } else {
            None
        };

        CostSummary {
            currency: rates.currency.clone(),
            total_per_hour: format_currency(total_cost, &rates.currency),
            cpu_per_hour: format_currency(cpu_cost, &rates.currency),
            memory_per_hour: format_currency(memory_cost, &rates.currency),
            gpu_per_hour: gpu_cost.map(|c| format_currency(c, &rates.currency)),
            projected_monthly: format_currency(project_monthly(total_cost), &rates.currency),
            efficiency_score: efficiency,
        }
    }

    fn calculate_gpu_cost(
        &self,
        gpu_stats: &[GpuResourceStats],
        rates: &PricingRates,
    ) -> Option<Decimal> {
        if gpu_stats.is_empty() {
            return None;
        }

        let total: Decimal = gpu_stats
            .iter()
            .map(|gpu| self.find_gpu_rate(&gpu.model, rates).unwrap_or(Decimal::ZERO))
            .sum();

        Some(total)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
