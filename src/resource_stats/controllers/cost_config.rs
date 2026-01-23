//! CostConfig controller - reconciles pricing configurations

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use tracing::{error, info, warn};

use crate::resource_stats::cost::cloud::aws::AwsPricingProvider;
use crate::resource_stats::cost::cloud::azure::AzurePricingProvider;
use crate::resource_stats::cost::cloud::gcp::GcpPricingProvider;
use crate::resource_stats::cost::cloud::CloudPricingProvider;
use crate::resource_stats::cost::static_pricing::StaticPricingCalculator;
use crate::resource_stats::types::cost_config::{
    CloudProvider, Condition, CostConfig, CostConfigPhase, CostConfigStatus, CostSource,
};
use crate::resource_stats::ResourceStatsContext;
use crate::resource_stats::ResourceStatsError;

/// Reconciles CostConfig resources
pub async fn reconcile(
    config: Arc<CostConfig>,
    ctx: Arc<ResourceStatsContext>,
) -> Result<Action, ResourceStatsError> {
    let name = config.name_any();
    let namespace = config.namespace().unwrap_or_else(|| "default".to_string());

    info!("Reconciling CostConfig {}/{}", namespace, name);

    let api: Api<CostConfig> = Api::namespaced(ctx.client.clone(), &namespace);

    // Update status to Syncing
    update_phase(&api, &name, CostConfigPhase::Syncing, None).await?;

    // Process based on source type
    let result = match &config.spec.source {
        CostSource::Static => process_static_pricing(&config, &ctx).await,
        CostSource::Cloud => process_cloud_pricing(&config, &ctx).await,
        CostSource::Hybrid => {
            // Try cloud first, fall back to static
            match process_cloud_pricing(&config, &ctx).await {
                Ok(_) => Ok(()),
                Err(e) => {
                    warn!(
                        "Cloud pricing failed for {}/{}, falling back to static: {}",
                        namespace, name, e
                    );
                    process_static_pricing(&config, &ctx).await
                }
            }
        }
    };

    match result {
        Ok(_) => {
            update_status_ready(&api, &name).await?;
            info!("CostConfig {}/{} is Ready", namespace, name);
        }
        Err(e) => {
            update_phase(&api, &name, CostConfigPhase::Failed, Some(e.to_string())).await?;
            error!("CostConfig {}/{} failed: {}", namespace, name, e);
            return Err(e);
        }
    }

    // Requeue based on refresh interval
    let refresh_secs = parse_duration(&config.spec.refresh_interval).unwrap_or(3600);
    Ok(Action::requeue(Duration::from_secs(refresh_secs)))
}

/// Error policy for CostConfig reconciliation
pub fn error_policy(
    _config: Arc<CostConfig>,
    error: &ResourceStatsError,
    _ctx: Arc<ResourceStatsContext>,
) -> Action {
    error!("CostConfig reconcile error: {}", error);
    Action::requeue(Duration::from_secs(60))
}

/// Process static pricing configuration
async fn process_static_pricing(
    config: &CostConfig,
    ctx: &ResourceStatsContext,
) -> Result<(), ResourceStatsError> {
    let static_pricing = config.spec.static_pricing.as_ref().ok_or_else(|| {
        ResourceStatsError::Config("Static pricing source requires staticPricing spec".into())
    })?;

    info!(
        "Processing static pricing: CPU={}/core/hr, Memory={}/GiB/hr",
        static_pricing.cpu_per_core_hour, static_pricing.memory_per_gib_hour
    );

    // Update the cost calculator with new rates
    if let Some(calculator) = ctx
        .cost_calculator
        .as_any()
        .downcast_ref::<StaticPricingCalculator>()
    {
        calculator.update_from_config(config).await.map_err(|e| {
            ResourceStatsError::Cost(crate::resource_stats::cost::CostError::InvalidPricing(
                e.to_string(),
            ))
        })?;
    }

    Ok(())
}

/// Process cloud pricing configuration
async fn process_cloud_pricing(
    config: &CostConfig,
    ctx: &ResourceStatsContext,
) -> Result<(), ResourceStatsError> {
    let provider_type = config.spec.cloud_provider.as_ref().ok_or_else(|| {
        ResourceStatsError::Config("Cloud pricing source requires cloudProvider".into())
    })?;

    let region = config.spec.region.as_deref().unwrap_or_else(|| {
        // Default region based on provider
        match provider_type {
            CloudProvider::Gcp => "us-central1",
            CloudProvider::Aws => "us-east-1",
            CloudProvider::Azure => "eastus",
        }
    });

    info!(
        "Fetching cloud pricing from {:?} for region {}",
        provider_type, region
    );

    // Create the appropriate cloud pricing provider
    let pricing_provider: Box<dyn CloudPricingProvider> = match provider_type {
        CloudProvider::Gcp => {
            // Get project ID from config or environment
            let project_id = std::env::var("GOOGLE_PROJECT_ID")
                .or_else(|_| std::env::var("GCP_PROJECT_ID"))
                .unwrap_or_else(|_| "default-project".to_string());
            Box::new(GcpPricingProvider::new(project_id))
        }
        CloudProvider::Aws => {
            Box::new(AwsPricingProvider::new(region.to_string()))
        }
        CloudProvider::Azure => {
            let provider = AzurePricingProvider::new();
            // Optionally configure with subscription ID
            if let Ok(subscription_id) = std::env::var("AZURE_SUBSCRIPTION_ID") {
                Box::new(provider.with_subscription(subscription_id))
            } else {
                Box::new(provider)
            }
        }
    };

    // Validate credentials if needed
    if let Err(e) = pricing_provider.validate_credentials().await {
        warn!(
            "Cloud credentials validation warning for {:?}: {}",
            provider_type, e
        );
        // Continue anyway - some providers don't require credentials for public pricing
    }

    // Fetch pricing rates
    let rates = pricing_provider.fetch_pricing(region).await.map_err(|e| {
        ResourceStatsError::Cost(crate::resource_stats::cost::CostError::CloudApi(format!(
            "Failed to fetch {:?} pricing: {}",
            provider_type, e
        )))
    })?;

    info!(
        "Fetched {} pricing: CPU={}/core/hr, Memory={}/GiB/hr, {} GPU rates",
        pricing_provider.name(),
        rates.cpu_per_core_hour,
        rates.memory_per_gib_hour,
        rates.gpu_pricing.len()
    );

    // Update the cost calculator with cloud rates
    if let Some(calculator) = ctx
        .cost_calculator
        .as_any()
        .downcast_ref::<StaticPricingCalculator>()
    {
        calculator.update_rates(rates).await;
    }

    Ok(())
}

/// Update CostConfig phase
async fn update_phase(
    api: &Api<CostConfig>,
    name: &str,
    phase: CostConfigPhase,
    message: Option<String>,
) -> Result<(), ResourceStatsError> {
    let status = CostConfigStatus {
        phase: phase.clone(),
        active: phase == CostConfigPhase::Ready,
        message,
        last_sync_time: Some(Utc::now().to_rfc3339()),
        ..Default::default()
    };

    let patch = serde_json::json!({ "status": status });
    api.patch_status(name, &PatchParams::apply("resource-stats-operator"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

/// Update CostConfig status to Ready with full details
async fn update_status_ready(api: &Api<CostConfig>, name: &str) -> Result<(), ResourceStatsError> {
    let now = Utc::now().to_rfc3339();

    let status = CostConfigStatus {
        phase: CostConfigPhase::Ready,
        active: true,
        last_sync_time: Some(now.clone()),
        pricing_version: Some(now),
        message: None,
        conditions: vec![Condition {
            r#type: "Ready".to_string(),
            status: "True".to_string(),
            reason: Some("PricingConfigured".to_string()),
            message: Some("Pricing configuration loaded successfully".to_string()),
            last_transition_time: Utc::now().to_rfc3339(),
        }],
        observed_generation: None,
    };

    let patch = serde_json::json!({ "status": status });
    api.patch_status(name, &PatchParams::apply("resource-stats-operator"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

/// Parse duration string (e.g., "1h", "30m", "300s") to seconds
fn parse_duration(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.ends_with('h') {
        s.trim_end_matches('h').parse::<u64>().ok().map(|h| h * 3600)
    } else if s.ends_with('m') {
        s.trim_end_matches('m').parse::<u64>().ok().map(|m| m * 60)
    } else if s.ends_with('s') {
        s.trim_end_matches('s').parse::<u64>().ok()
    } else {
        s.parse::<u64>().ok()
    }
}
