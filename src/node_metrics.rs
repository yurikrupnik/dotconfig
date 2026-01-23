/// Kubernetes operator using trait abstractions for testability
use async_trait::async_trait;
use k8s_openapi::api::core::v1::Node;
use std::collections::BTreeMap;
use thiserror::Error;

/// Error types for Kubernetes operations
#[derive(Error, Debug)]
pub enum KubeError {
    #[error("Failed to connect to Kubernetes cluster: {0}")]
    Connection(String),

    #[error("Failed to list resources: {0}")]
    ListError(String),

    #[error("Failed to make API request: {0}")]
    RequestError(String),

    #[error("Failed to deserialize response: {0}")]
    DeserializationError(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid configuration: {0}")]
    ConfigError(String),
}

/// Metrics for a Kubernetes node
#[derive(Debug, Clone)]
pub struct NodeMetrics {
    pub cpu_usage_nano_cores: usize,
    pub memory_usage_bytes: usize,
}

/// Summary of a Kubernetes node including metrics and allocatable resources
#[derive(Debug, Clone)]
pub struct NodeSummary {
    pub name: String,
    pub metrics: NodeMetrics,
    pub allocatable: BTreeMap<String, String>,
}

/// Trait for Kubernetes client operations
#[async_trait]
pub trait KubeClient: Send + Sync {
    /// Lists all nodes in the cluster
    async fn list_nodes(&self) -> Result<Vec<Node>, KubeError>;

    /// Gets node metrics by querying the kubelet stats endpoint
    async fn get_node_metrics(&self, node_name: &str) -> Result<NodeMetrics, KubeError>;

    /// Gets a summary of all nodes with their metrics
    async fn get_nodes_summary(&self) -> Result<Vec<NodeSummary>, KubeError> {
        let nodes = self.list_nodes().await?;
        let mut summaries = Vec::new();

        for node in nodes {
            let name = node
                .metadata
                .name
                .clone()
                .ok_or_else(|| KubeError::ConfigError("Node has no name".into()))?;

            let metrics = self.get_node_metrics(&name).await?;

            let allocatable = node
                .status
                .and_then(|s| s.allocatable)
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| (k, v.0))
                .collect();

            summaries.push(NodeSummary {
                name,
                metrics,
                allocatable,
            });
        }

        Ok(summaries)
    }
}

/// Real Kubernetes client implementation using kube-rs
pub struct RealKubeClient {
    client: kube::Client,
}

impl RealKubeClient {
    /// Creates a new Kubernetes client using the default configuration
    pub async fn try_default() -> Result<Self, KubeError> {
        let client = kube::Client::try_default()
            .await
            .map_err(|e| KubeError::Connection(e.to_string()))?;

        Ok(Self { client })
    }
}

#[async_trait]
impl KubeClient for RealKubeClient {
    async fn list_nodes(&self) -> Result<Vec<Node>, KubeError> {
        use kube::api::ListParams;
        use kube::Api;

        let api: Api<Node> = Api::all(self.client.clone());
        let nodes = api
            .list(&ListParams::default())
            .await
            .map_err(|e| KubeError::ListError(e.to_string()))?;

        Ok(nodes.items)
    }

    async fn get_node_metrics(&self, node_name: &str) -> Result<NodeMetrics, KubeError> {
        use serde::Deserialize;

        // Query node stats from kubelet admin endpoint
        let url = format!("/api/v1/nodes/{}/proxy/stats/summary", node_name);
        let req = http::Request::get(url)
            .body(Vec::new())
            .map_err(|e| KubeError::RequestError(e.to_string()))?;

        let resp: serde_json::Value = self
            .client
            .request(req)
            .await
            .map_err(|e| KubeError::RequestError(e.to_string()))?;

        // Extract node metrics from the response
        let summary = resp
            .get("node")
            .ok_or_else(|| KubeError::NotFound("node summary not found in response".into()))?;

        #[derive(Deserialize)]
        struct CpuMetric {
            #[serde(rename = "usageNanoCores")]
            usage_nano_cores: usize,
        }

        #[derive(Deserialize)]
        struct MemoryMetric {
            #[serde(rename = "usageBytes")]
            usage_bytes: usize,
        }

        #[derive(Deserialize)]
        struct Metrics {
            cpu: CpuMetric,
            memory: MemoryMetric,
        }

        let metrics: Metrics = serde_json::from_value(summary.clone())
            .map_err(|e| KubeError::DeserializationError(e.to_string()))?;

        Ok(NodeMetrics {
            cpu_usage_nano_cores: metrics.cpu.usage_nano_cores,
            memory_usage_bytes: metrics.memory.usage_bytes,
        })
    }
}

/// Displays node metrics in a formatted table
pub fn print_table(summaries: Vec<NodeSummary>) {
    use headers::*;

    // Calculate column widths
    let w_used_mem = USED_MEM.len() + 4;
    let w_used_cpu = USED_CPU.len() + 2;
    let w_percent_mem = PERCENT_MEM.len() + 2;
    let w_percent_cpu = PERCENT_CPU.len() + 4;

    // Width of name column accommodates the longest node name
    let w_name = {
        let max_name_width = summaries
            .iter()
            .map(|summary| summary.name.len())
            .max()
            .unwrap_or(0)
            .max(NAME.len());
        max_name_width + 4
    };

    // Print header
    println!(
        "{NAME:w_name$} {USED_MEM:w_used_mem$} {PERCENT_MEM:w_percent_mem$} {USED_CPU:w_used_cpu$} {PERCENT_CPU:w_percent_cpu$}"
    );

    // Print each node's metrics
    for summary in summaries {
        let name = &summary.name;

        // Parse memory allocatable (in Ki)
        let mem_total = summary
            .allocatable
            .get("memory")
            .and_then(|mem| mem.trim_end_matches("Ki").parse::<usize>().ok())
            .unwrap_or(1);

        // Parse CPU allocatable (whole cores)
        let cpu_total = summary
            .allocatable
            .get("cpu")
            .and_then(|cpu| cpu.parse::<usize>().ok())
            .unwrap_or(1);

        let (percent_mem, used_mem) =
            convert_memory_to_stat(summary.metrics.memory_usage_bytes, mem_total);
        let (percent_cpu, used_cpu) =
            convert_cpu_to_stat(summary.metrics.cpu_usage_nano_cores, cpu_total);

        println!("{name:w_name$} {used_mem:<w_used_mem$} {percent_mem:<w_percent_mem$} {used_cpu:<w_used_cpu$} {percent_cpu:<w_percent_cpu$}");
    }
}

/// Convert memory usage to human-readable format
fn convert_memory_to_stat(usage_bytes: usize, alloc_kibibytes: usize) -> (String, String) {
    // 1 MiB = 2^20 bytes
    let mem_mib = usage_bytes as f64 / (1 << 20) as f64;
    // 1 MiB = 2^10 KiB
    let alloc_mib = alloc_kibibytes as f64 / (1 << 10) as f64;
    let used_percent = ((mem_mib / alloc_mib) * 100.0) as usize;

    (
        format!("{}%", used_percent),
        format!("{}Mi", mem_mib as usize),
    )
}

/// Convert CPU usage to human-readable format
fn convert_cpu_to_stat(usage_nano_cores: usize, alloc_cores: usize) -> (String, String) {
    // 1 millicore = 1000th of a CPU
    // 1 nanocore = 1 billionth of a CPU
    // Convert nano to milli: divide by 1,000,000
    let cpu_millicores = (usage_nano_cores / 1_000_000) as f64;
    // Convert cores to millicores
    let alloc_millicores = (alloc_cores * 1000) as f64;
    let used_percent = ((cpu_millicores / alloc_millicores) * 100.0) as usize;

    (
        format!("{}%", used_percent),
        format!("{}m", cpu_millicores as usize),
    )
}

/// Runs the operator with the given Kubernetes client
pub async fn run_operator(client: &dyn KubeClient) -> Result<(), KubeError> {
    let summaries = client.get_nodes_summary().await?;
    print_table(summaries);
    Ok(())
}

/// Namespaces table header constants
pub mod headers {
    pub const NAME: &str = "NAME";
    pub const USED_MEM: &str = "MEMORY(bytes)";
    pub const USED_CPU: &str = "CPU(cores)";
    pub const PERCENT_MEM: &str = "MEMORY%";
    pub const PERCENT_CPU: &str = "CPU%";
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a real Kubernetes client
    let client = RealKubeClient::try_default().await?;

    // Run the operator with the real client
    run_operator(&client).await?;

    Ok(())
}
