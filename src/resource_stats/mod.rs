//! Resource Stats Operator
//!
//! Collects CPU/memory/GPU metrics, calculates costs, and exposes via web/TUI.

pub mod cache;
pub mod controllers;
pub mod cost;
pub mod metrics;
pub mod types;

#[cfg(feature = "web-ui")]
pub mod web;

#[cfg(feature = "tui")]
pub mod tui;

use std::sync::Arc;

use kube::Client;

use crate::resource_stats::cache::MetricsCache;
use crate::resource_stats::cost::CostCalculator;
use crate::resource_stats::metrics::{GpuCollector, MetricsCollector};
use crate::resource_stats::types::resource_stats::ResourceStatsStatus;

/// Shared state for UI layer
pub struct UiState {
    /// Cached cluster stats
    pub cluster_stats: tokio::sync::RwLock<Option<ResourceStatsStatus>>,
    /// Last update timestamp
    pub last_update: tokio::sync::RwLock<Option<chrono::DateTime<chrono::Utc>>>,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            cluster_stats: tokio::sync::RwLock::new(None),
            last_update: tokio::sync::RwLock::new(None),
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Context for resource stats operator
pub struct ResourceStatsContext {
    /// Kubernetes client
    pub client: Client,
    /// Metrics collector
    pub metrics_collector: Arc<dyn MetricsCollector>,
    /// Cost calculator
    pub cost_calculator: Arc<dyn CostCalculator>,
    /// GPU collectors (one per vendor)
    pub gpu_collectors: Vec<Arc<dyn GpuCollector>>,
    /// Metrics cache
    pub cache: Arc<MetricsCache>,
    /// Shared UI state
    pub ui_state: Arc<UiState>,
}

/// Operator-level error types
#[derive(thiserror::Error, Debug)]
pub enum ResourceStatsError {
    #[error("Kubernetes API error: {0}")]
    KubeApi(#[from] kube::Error),

    #[error("Metrics error: {0}")]
    Metrics(#[from] metrics::MetricsError),

    #[error("Cost calculation error: {0}")]
    Cost(#[from] cost::CostError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),
}
