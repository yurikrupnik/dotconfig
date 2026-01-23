//! Metrics collection module

pub mod gpu;
pub mod node;

use async_trait::async_trait;

use crate::resource_stats::types::resource_stats::{
    GpuResourceStats, NodeResourceStats, PodResourceStats, ResourceSnapshot,
};

/// Error type for metrics collection
#[derive(thiserror::Error, Debug)]
pub enum MetricsError {
    #[error("Kubernetes API error: {0}")]
    KubeApi(#[from] kube::Error),

    #[error("Metrics unavailable: {0}")]
    Unavailable(String),

    #[error("GPU metrics error: {0}")]
    Gpu(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("HTTP error: {0}")]
    Http(String),
}

/// Trait for collecting node/pod metrics
#[async_trait]
pub trait MetricsCollector: Send + Sync {
    /// Collect a snapshot of current resource usage
    async fn collect_snapshot(&self) -> Result<ResourceSnapshot, MetricsError>;

    /// Collect per-node metrics
    async fn collect_node_metrics(&self) -> Result<Vec<NodeResourceStats>, MetricsError>;

    /// Collect per-pod metrics (optionally filtered by namespace)
    async fn collect_pod_metrics(
        &self,
        namespace: Option<&str>,
    ) -> Result<Vec<PodResourceStats>, MetricsError>;
}

/// Trait for GPU-specific metrics collection
#[async_trait]
pub trait GpuCollector: Send + Sync {
    /// Check if GPU vendor is available on this node
    fn is_available(&self) -> bool;

    /// Get GPU vendor name
    fn vendor(&self) -> &'static str;

    /// Collect GPU metrics from all available GPUs
    async fn collect(&self) -> Result<Vec<GpuResourceStats>, MetricsError>;
}
