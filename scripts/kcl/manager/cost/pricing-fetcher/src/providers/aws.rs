//! AWS Price List API
//! Uses the AWS SDK for Pricing API

use anyhow::Result;

use super::ProviderPricing;
use crate::{DatabasePricing, RegistryPricing, StoragePricing};

/// Map GCP region to AWS region
fn map_region(gcp_region: &str) -> &'static str {
    match gcp_region {
        "us-central1" => "us-east-1",
        "us-east1" => "us-east-1",
        "us-west1" => "us-west-1",
        "europe-west1" => "eu-west-1",
        "asia-east1" => "ap-northeast-1",
        _ => "us-east-1",
    }
}

pub async fn fetch_pricing(region: &str) -> Result<ProviderPricing> {
    let aws_region = map_region(region);

    // AWS Pricing API is only available in us-east-1 and ap-south-1
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new("us-east-1"))
        .load()
        .await;

    let pricing_client = aws_sdk_pricing::Client::new(&config);

    // Fetch ECR pricing
    let registry = fetch_ecr_pricing(&pricing_client, aws_region)
        .await
        .unwrap_or_else(|_| default_registry());

    // Fetch RDS pricing
    let database = fetch_rds_pricing(&pricing_client, aws_region)
        .await
        .unwrap_or_else(|_| default_database());

    // Fetch S3 pricing
    let storage = fetch_s3_pricing(&pricing_client, aws_region)
        .await
        .unwrap_or_else(|_| default_storage());

    Ok(ProviderPricing {
        registry,
        database,
        storage,
    })
}

async fn fetch_ecr_pricing(
    client: &aws_sdk_pricing::Client,
    region: &str,
) -> Result<RegistryPricing> {
    // ECR pricing filter
    let response = client
        .get_products()
        .service_code("AmazonECR")
        .filters(
            aws_sdk_pricing::types::Filter::builder()
                .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                .field("regionCode")
                .value(region)
                .build()?,
        )
        .send()
        .await?;

    let mut storage_price = 0.10;

    let price_list = response.price_list();
    for price_str in price_list {
        // Parse JSON price data
        if let Ok(price_data) = serde_json::from_str::<serde_json::Value>(price_str) {
            if let Some(terms) = price_data.get("terms").and_then(|t| t.get("OnDemand")) {
                for (_, term) in terms.as_object().unwrap_or(&serde_json::Map::new()) {
                    if let Some(price_dimensions) = term.get("priceDimensions") {
                        for (_, dim) in
                            price_dimensions.as_object().unwrap_or(&serde_json::Map::new())
                        {
                            if let Some(desc) = dim.get("description") {
                                if desc.as_str().unwrap_or("").contains("storage") {
                                    if let Some(price_per_unit) = dim.get("pricePerUnit") {
                                        if let Some(usd) = price_per_unit.get("USD") {
                                            if let Ok(price) =
                                                usd.as_str().unwrap_or("0").parse::<f64>()
                                            {
                                                storage_price = price;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(RegistryPricing {
        storage_per_gb_month: storage_price,
        egress_per_gb: 0.09,
        pull_per_1000: 0.0,
        vulnerability_scan_per_image: 0.09, // ECR image scanning
    })
}

async fn fetch_rds_pricing(
    client: &aws_sdk_pricing::Client,
    region: &str,
) -> Result<DatabasePricing> {
    // RDS PostgreSQL pricing filter
    let response = client
        .get_products()
        .service_code("AmazonRDS")
        .filters(
            aws_sdk_pricing::types::Filter::builder()
                .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                .field("regionCode")
                .value(region)
                .build()?,
        )
        .filters(
            aws_sdk_pricing::types::Filter::builder()
                .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                .field("databaseEngine")
                .value("PostgreSQL")
                .build()?,
        )
        .send()
        .await?;

    let mut cpu_price = 0.04;
    let mut storage_price = 0.115;

    let price_list = response.price_list();
    for price_str in price_list {
        if let Ok(price_data) = serde_json::from_str::<serde_json::Value>(price_str) {
            // Look for db.m5.large or similar general purpose instance
            if let Some(product) = price_data.get("product") {
                if let Some(attrs) = product.get("attributes") {
                    let instance_type = attrs
                        .get("instanceType")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    if instance_type.contains("db.m5") || instance_type.contains("db.m6g") {
                        if let Some(terms) =
                            price_data.get("terms").and_then(|t| t.get("OnDemand"))
                        {
                            for (_, term) in
                                terms.as_object().unwrap_or(&serde_json::Map::new())
                            {
                                if let Some(price_dimensions) = term.get("priceDimensions") {
                                    for (_, dim) in price_dimensions
                                        .as_object()
                                        .unwrap_or(&serde_json::Map::new())
                                    {
                                        if let Some(price_per_unit) = dim.get("pricePerUnit") {
                                            if let Some(usd) = price_per_unit.get("USD") {
                                                if let Ok(price) =
                                                    usd.as_str().unwrap_or("0").parse::<f64>()
                                                {
                                                    // db.m5.large has 2 vCPUs
                                                    cpu_price = price / 2.0;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let _ = storage_price; // Suppress unused warning

    Ok(DatabasePricing {
        cpu_per_hour: cpu_price,
        memory_per_gb_hour: cpu_price / 5.0, // Approximate
        storage_per_gb_month: storage_price,
        iops_per_1000_hour: 0.10,
        backup_per_gb_month: 0.095,
        ha_multiplier: 2.0,
    })
}

async fn fetch_s3_pricing(
    client: &aws_sdk_pricing::Client,
    region: &str,
) -> Result<StoragePricing> {
    let response = client
        .get_products()
        .service_code("AmazonS3")
        .filters(
            aws_sdk_pricing::types::Filter::builder()
                .r#type(aws_sdk_pricing::types::FilterType::TermMatch)
                .field("regionCode")
                .value(region)
                .build()?,
        )
        .send()
        .await?;

    let mut storage_price = 0.023;
    let egress_price = 0.09;

    let price_list = response.price_list();
    for price_str in price_list {
        if let Ok(price_data) = serde_json::from_str::<serde_json::Value>(price_str) {
            if let Some(product) = price_data.get("product") {
                if let Some(attrs) = product.get("attributes") {
                    let storage_class = attrs
                        .get("storageClass")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    if storage_class == "General Purpose" {
                        if let Some(terms) =
                            price_data.get("terms").and_then(|t| t.get("OnDemand"))
                        {
                            for (_, term) in
                                terms.as_object().unwrap_or(&serde_json::Map::new())
                            {
                                if let Some(price_dimensions) = term.get("priceDimensions") {
                                    for (_, dim) in price_dimensions
                                        .as_object()
                                        .unwrap_or(&serde_json::Map::new())
                                    {
                                        if let Some(price_per_unit) = dim.get("pricePerUnit") {
                                            if let Some(usd) = price_per_unit.get("USD") {
                                                if let Ok(price) =
                                                    usd.as_str().unwrap_or("0").parse::<f64>()
                                                {
                                                    storage_price = price;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
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
        egress_per_gb: 0.09,
        pull_per_1000: 0.0,
        vulnerability_scan_per_image: 0.09,
    }
}

fn default_database() -> DatabasePricing {
    DatabasePricing {
        cpu_per_hour: 0.04,
        memory_per_gb_hour: 0.008,
        storage_per_gb_month: 0.115,
        iops_per_1000_hour: 0.10,
        backup_per_gb_month: 0.095,
        ha_multiplier: 2.0,
    }
}

fn default_storage() -> StoragePricing {
    StoragePricing {
        storage_per_gb_month: 0.023,
        egress_per_gb: 0.09,
        operations_per_10k: 0.05,
    }
}
