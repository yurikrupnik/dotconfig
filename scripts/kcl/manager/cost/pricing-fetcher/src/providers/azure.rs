//! Azure Retail Prices API
//! https://learn.microsoft.com/en-us/rest/api/cost-management/retail-prices/azure-retail-prices

use anyhow::Result;
use serde::Deserialize;

use super::ProviderPricing;
use crate::{DatabasePricing, RegistryPricing, StoragePricing};

const AZURE_RETAIL_PRICES_API: &str = "https://prices.azure.com/api/retail/prices";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AzurePriceResponse {
    items: Vec<AzurePriceItem>,
    next_page_link: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzurePriceItem {
    sku_name: String,
    service_name: String,
    product_name: String,
    meter_name: String,
    unit_of_measure: String,
    unit_price: f64,
    arm_region_name: String,
}

/// Map GCP region to Azure region
fn map_region(gcp_region: &str) -> &'static str {
    match gcp_region {
        "us-central1" => "eastus",
        "us-east1" => "eastus",
        "us-west1" => "westus",
        "europe-west1" => "westeurope",
        "asia-east1" => "eastasia",
        _ => "eastus",
    }
}

pub async fn fetch_pricing(region: &str) -> Result<ProviderPricing> {
    let azure_region = map_region(region);
    let client = reqwest::Client::new();

    // Fetch ACR pricing
    let registry = fetch_acr_pricing(&client, azure_region).await.unwrap_or(RegistryPricing {
        storage_per_gb_month: 0.167,
        egress_per_gb: 0.087,
        pull_per_1000: 0.0,
        vulnerability_scan_per_image: 0.0,
    });

    // Fetch Azure Database for PostgreSQL pricing
    let database = fetch_postgres_pricing(&client, azure_region).await.unwrap_or(DatabasePricing {
        cpu_per_hour: 0.045,
        memory_per_gb_hour: 0.0075,
        storage_per_gb_month: 0.115,
        iops_per_1000_hour: 0.0,
        backup_per_gb_month: 0.095,
        ha_multiplier: 1.5,
    });

    // Fetch Azure Blob Storage pricing
    let storage = fetch_blob_pricing(&client, azure_region).await.unwrap_or(StoragePricing {
        storage_per_gb_month: 0.018,
        egress_per_gb: 0.087,
        operations_per_10k: 0.05,
    });

    Ok(ProviderPricing {
        registry,
        database,
        storage,
    })
}

async fn fetch_acr_pricing(client: &reqwest::Client, region: &str) -> Result<RegistryPricing> {
    let filter = format!(
        "serviceName eq 'Container Registry' and armRegionName eq '{}'",
        region
    );

    let response: AzurePriceResponse = client
        .get(AZURE_RETAIL_PRICES_API)
        .query(&[("$filter", filter)])
        .send()
        .await?
        .json()
        .await?;

    let mut storage_price = 0.167;

    for item in response.items {
        if item.meter_name.contains("Standard Registry Unit") {
            // ACR Standard tier - includes 100GB
            storage_price = item.unit_price / 100.0; // Convert to per-GB
        }
    }

    Ok(RegistryPricing {
        storage_per_gb_month: storage_price,
        egress_per_gb: 0.087, // Standard egress
        pull_per_1000: 0.0,
        vulnerability_scan_per_image: 0.0, // Defender pricing separate
    })
}

async fn fetch_postgres_pricing(client: &reqwest::Client, region: &str) -> Result<DatabasePricing> {
    let filter = format!(
        "serviceName eq 'Azure Database for PostgreSQL' and armRegionName eq '{}'",
        region
    );

    let response: AzurePriceResponse = client
        .get(AZURE_RETAIL_PRICES_API)
        .query(&[("$filter", filter)])
        .send()
        .await?
        .json()
        .await?;

    let mut cpu_hour = 0.045;
    let mut storage_gb_month = 0.115;

    for item in response.items {
        // Flexible Server pricing
        if item.product_name.contains("Flexible Server")
            && item.sku_name.contains("General Purpose")
        {
            if item.meter_name.contains("vCore") {
                // Convert from per-hour vCore to approximate CPU cost
                cpu_hour = item.unit_price;
            }
        }
        if item.meter_name.contains("Storage") && item.unit_of_measure.contains("GB") {
            storage_gb_month = item.unit_price;
        }
    }

    Ok(DatabasePricing {
        cpu_per_hour: cpu_hour,
        memory_per_gb_hour: cpu_hour / 6.0, // Approximate memory cost
        storage_per_gb_month: storage_gb_month,
        iops_per_1000_hour: 0.0,
        backup_per_gb_month: storage_gb_month * 0.8, // Backup is ~80% of storage
        ha_multiplier: 1.5, // Zone-redundant HA
    })
}

async fn fetch_blob_pricing(client: &reqwest::Client, region: &str) -> Result<StoragePricing> {
    let filter = format!(
        "serviceName eq 'Storage' and armRegionName eq '{}' and skuName eq 'Standard LRS'",
        region
    );

    let response: AzurePriceResponse = client
        .get(AZURE_RETAIL_PRICES_API)
        .query(&[("$filter", filter)])
        .send()
        .await?
        .json()
        .await?;

    let mut storage_price = 0.018;
    let mut egress_price = 0.087;

    for item in response.items {
        if item.product_name.contains("Blob Storage")
            && item.meter_name.contains("Hot LRS Data Stored")
        {
            storage_price = item.unit_price;
        }
        if item.meter_name.contains("Data Transfer Out") {
            egress_price = item.unit_price;
        }
    }

    Ok(StoragePricing {
        storage_per_gb_month: storage_price,
        egress_per_gb: egress_price,
        operations_per_10k: 0.05,
    })
}
