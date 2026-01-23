//! GCP Cloud Billing API
//! Requires Application Default Credentials or service account

use anyhow::Result;
use serde::Deserialize;

use super::ProviderPricing;
use crate::{DatabasePricing, RegistryPricing, StoragePricing};

// GCP Cloud Billing Catalog API
const GCP_CATALOG_API: &str = "https://cloudbilling.googleapis.com/v1/services";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceListResponse {
    services: Vec<Service>,
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Service {
    name: String,
    service_id: String,
    display_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkuListResponse {
    skus: Vec<Sku>,
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sku {
    name: String,
    sku_id: String,
    description: String,
    category: SkuCategory,
    service_regions: Vec<String>,
    pricing_info: Vec<PricingInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkuCategory {
    service_display_name: String,
    resource_family: String,
    resource_group: String,
    usage_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PricingInfo {
    pricing_expression: PricingExpression,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PricingExpression {
    usage_unit: String,
    tiered_rates: Vec<TieredRate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TieredRate {
    start_usage_amount: f64,
    unit_price: UnitPrice,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnitPrice {
    currency_code: String,
    units: Option<String>,
    nanos: Option<i64>,
}

impl UnitPrice {
    fn to_dollars(&self) -> f64 {
        let units: f64 = self.units.as_ref().and_then(|u| u.parse().ok()).unwrap_or(0.0);
        let nanos: f64 = self.nanos.unwrap_or(0) as f64 / 1_000_000_000.0;
        units + nanos
    }
}

pub async fn fetch_pricing(region: &str) -> Result<ProviderPricing> {
    // GCP requires OAuth2 authentication
    let auth = gcp_auth::provider().await?;
    let token = auth.token(&["https://www.googleapis.com/auth/cloud-platform"]).await?;

    let client = reqwest::Client::new();

    // Fetch Artifact Registry pricing
    let registry = fetch_artifact_registry_pricing(&client, token.as_str(), region)
        .await
        .unwrap_or_else(|_| default_registry());

    // Fetch Cloud SQL pricing
    let database = fetch_cloudsql_pricing(&client, token.as_str(), region)
        .await
        .unwrap_or_else(|_| default_database());

    // Fetch Cloud Storage pricing
    let storage = fetch_gcs_pricing(&client, token.as_str(), region)
        .await
        .unwrap_or_else(|_| default_storage());

    Ok(ProviderPricing {
        registry,
        database,
        storage,
    })
}

async fn fetch_artifact_registry_pricing(
    client: &reqwest::Client,
    token: &str,
    _region: &str,
) -> Result<RegistryPricing> {
    // Service ID for Artifact Registry
    let service = "services/artifactregistry.googleapis.com";

    let response: SkuListResponse = client
        .get(format!("{}/{}/skus", GCP_CATALOG_API, service))
        .bearer_auth(token)
        .send()
        .await?
        .json()
        .await?;

    let mut storage_price = 0.10;

    for sku in response.skus {
        if sku.description.contains("Storage") {
            if let Some(pricing) = sku.pricing_info.first() {
                if let Some(rate) = pricing.pricing_expression.tiered_rates.first() {
                    let price = rate.unit_price.to_dollars();
                    if price > 0.0 {
                        storage_price = price;
                    }
                }
            }
        }
    }

    Ok(RegistryPricing {
        storage_per_gb_month: storage_price,
        egress_per_gb: 0.12, // Standard networking egress
        pull_per_1000: 0.0,
        vulnerability_scan_per_image: 0.26, // Container Analysis pricing
    })
}

async fn fetch_cloudsql_pricing(
    client: &reqwest::Client,
    token: &str,
    _region: &str,
) -> Result<DatabasePricing> {
    // Service ID for Cloud SQL
    let service = "services/sqladmin.googleapis.com";

    let response: SkuListResponse = client
        .get(format!("{}/{}/skus", GCP_CATALOG_API, service))
        .bearer_auth(token)
        .send()
        .await?
        .json()
        .await?;

    let mut cpu_price = 0.0413;
    let mut memory_price = 0.007;
    let mut storage_price = 0.17;

    for sku in response.skus {
        // Look for PostgreSQL pricing
        if sku.description.contains("PostgreSQL") || sku.category.resource_group.contains("SQL") {
            if let Some(pricing) = sku.pricing_info.first() {
                if let Some(rate) = pricing.pricing_expression.tiered_rates.first() {
                    let price = rate.unit_price.to_dollars();
                    if price > 0.0 {
                        if sku.description.contains("CPU") || sku.description.contains("vCPU") {
                            cpu_price = price;
                        } else if sku.description.contains("RAM") || sku.description.contains("Memory") {
                            memory_price = price;
                        } else if sku.description.contains("Storage") || sku.description.contains("SSD") {
                            storage_price = price;
                        }
                    }
                }
            }
        }
    }

    Ok(DatabasePricing {
        cpu_per_hour: cpu_price,
        memory_per_gb_hour: memory_price,
        storage_per_gb_month: storage_price,
        iops_per_1000_hour: 0.0,
        backup_per_gb_month: 0.08,
        ha_multiplier: 2.0,
    })
}

async fn fetch_gcs_pricing(
    client: &reqwest::Client,
    token: &str,
    _region: &str,
) -> Result<StoragePricing> {
    // Service ID for Cloud Storage
    let service = "services/storage.googleapis.com";

    let response: SkuListResponse = client
        .get(format!("{}/{}/skus", GCP_CATALOG_API, service))
        .bearer_auth(token)
        .send()
        .await?
        .json()
        .await?;

    let mut storage_price = 0.020;
    let mut egress_price = 0.12;

    for sku in response.skus {
        if let Some(pricing) = sku.pricing_info.first() {
            if let Some(rate) = pricing.pricing_expression.tiered_rates.first() {
                let price = rate.unit_price.to_dollars();
                if price > 0.0 {
                    if sku.description.contains("Standard Storage") {
                        storage_price = price;
                    } else if sku.description.contains("Network Egress") {
                        egress_price = price;
                    }
                }
            }
        }
    }

    Ok(StoragePricing {
        storage_per_gb_month: storage_price,
        egress_per_gb: egress_price,
        operations_per_10k: 0.05,
    })
}

fn default_registry() -> RegistryPricing {
    RegistryPricing {
        storage_per_gb_month: 0.10,
        egress_per_gb: 0.12,
        pull_per_1000: 0.0,
        vulnerability_scan_per_image: 0.26,
    }
}

fn default_database() -> DatabasePricing {
    DatabasePricing {
        cpu_per_hour: 0.0413,
        memory_per_gb_hour: 0.007,
        storage_per_gb_month: 0.17,
        iops_per_1000_hour: 0.0,
        backup_per_gb_month: 0.08,
        ha_multiplier: 2.0,
    }
}

fn default_storage() -> StoragePricing {
    StoragePricing {
        storage_per_gb_month: 0.020,
        egress_per_gb: 0.12,
        operations_per_10k: 0.05,
    }
}
