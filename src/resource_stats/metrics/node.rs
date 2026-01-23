//! Node metrics collector using kubelet stats endpoint

use async_trait::async_trait;
use k8s_openapi::api::core::v1::{Node, Pod};
use kube::api::ListParams;
use kube::{Api, Client};
use serde::Deserialize;

use super::{GpuCollector, MetricsCollector, MetricsError};
use crate::resource_stats::types::resource_stats::{
    CpuMetrics, GpuMetrics, GpuResourceStats, MemoryMetrics, NodeResourceStats, PodResourceStats,
    ResourceSnapshot,
};

/// Node metrics collector using kubelet stats endpoint
pub struct NodeMetricsCollector {
    client: Client,
    gpu_collectors: Vec<Box<dyn GpuCollector>>,
}

impl NodeMetricsCollector {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            gpu_collectors: Vec::new(),
        }
    }

    pub fn with_gpu_collectors(mut self, collectors: Vec<Box<dyn GpuCollector>>) -> Self {
        self.gpu_collectors = collectors;
        self
    }

    /// Get raw node metrics from kubelet stats endpoint
    async fn get_node_stats(&self, node_name: &str) -> Result<RawNodeMetrics, MetricsError> {
        let url = format!("/api/v1/nodes/{}/proxy/stats/summary", node_name);
        let req = http::Request::get(&url)
            .body(Vec::new())
            .map_err(|e| MetricsError::Http(e.to_string()))?;

        let resp: serde_json::Value = self
            .client
            .request(req)
            .await
            .map_err(|e| MetricsError::KubeApi(e))?;

        let summary = resp
            .get("node")
            .ok_or_else(|| MetricsError::Unavailable("node summary not found".into()))?;

        let metrics: RawNodeMetrics = serde_json::from_value(summary.clone())
            .map_err(|e| MetricsError::Parse(e.to_string()))?;

        Ok(metrics)
    }

    /// List all nodes in the cluster
    async fn list_nodes(&self) -> Result<Vec<Node>, MetricsError> {
        let api: Api<Node> = Api::all(self.client.clone());
        let nodes = api.list(&ListParams::default()).await?;
        Ok(nodes.items)
    }

    /// List pods, optionally filtered by namespace
    async fn list_pods(&self, namespace: Option<&str>) -> Result<Vec<Pod>, MetricsError> {
        let api: Api<Pod> = match namespace {
            Some(ns) => Api::namespaced(self.client.clone(), ns),
            None => Api::all(self.client.clone()),
        };
        let pods = api.list(&ListParams::default()).await?;
        Ok(pods.items)
    }

    /// Collect GPU metrics from all available collectors
    async fn collect_gpu_metrics(&self) -> Result<Option<GpuMetrics>, MetricsError> {
        let mut all_gpu_stats: Vec<GpuResourceStats> = Vec::new();

        for collector in &self.gpu_collectors {
            if collector.is_available() {
                match collector.collect().await {
                    Ok(stats) => all_gpu_stats.extend(stats),
                    Err(e) => {
                        tracing::warn!("GPU collector {} failed: {}", collector.vendor(), e);
                    }
                }
            }
        }

        if all_gpu_stats.is_empty() {
            return Ok(None);
        }

        let total_gpus = all_gpu_stats.len() as i32;
        let used_gpus = all_gpu_stats
            .iter()
            .filter(|g| g.utilization_percent > 0.0)
            .count() as i32;
        let memory_used_bytes: i64 = all_gpu_stats.iter().map(|g| g.memory_used_bytes).sum();
        let memory_total_bytes: i64 = all_gpu_stats.iter().map(|g| g.memory_total_bytes).sum();
        let avg_utilization =
            all_gpu_stats.iter().map(|g| g.utilization_percent).sum::<f64>() / total_gpus as f64;

        Ok(Some(GpuMetrics {
            total_gpus,
            used_gpus,
            memory_used_bytes,
            memory_total_bytes,
            utilization_percent: avg_utilization,
        }))
    }
}

#[async_trait]
impl MetricsCollector for NodeMetricsCollector {
    async fn collect_snapshot(&self) -> Result<ResourceSnapshot, MetricsError> {
        let nodes = self.list_nodes().await?;

        let mut total_cpu_usage: i64 = 0;
        let mut total_cpu_capacity: i64 = 0;
        let mut total_mem_usage: i64 = 0;
        let mut total_mem_capacity: i64 = 0;

        for node in &nodes {
            let name = node
                .metadata
                .name
                .as_ref()
                .ok_or_else(|| MetricsError::Parse("Node has no name".into()))?;

            match self.get_node_stats(name).await {
                Ok(raw) => {
                    total_cpu_usage += (raw.cpu.usage_nano_cores / 1_000_000) as i64;
                    total_mem_usage += raw.memory.usage_bytes as i64;
                }
                Err(e) => {
                    tracing::warn!("Failed to get metrics for node {}: {}", name, e);
                }
            }

            if let Some(status) = &node.status {
                if let Some(alloc) = &status.allocatable {
                    if let Some(cpu) = alloc.get("cpu") {
                        let cores = parse_cpu_quantity(&cpu.0);
                        total_cpu_capacity += (cores * 1000.0) as i64;
                    }
                    if let Some(mem) = alloc.get("memory") {
                        total_mem_capacity += parse_memory_quantity(&mem.0);
                    }
                }
            }
        }

        let cpu_percent = if total_cpu_capacity > 0 {
            (total_cpu_usage as f64 / total_cpu_capacity as f64) * 100.0
        } else {
            0.0
        };

        let mem_percent = if total_mem_capacity > 0 {
            (total_mem_usage as f64 / total_mem_capacity as f64) * 100.0
        } else {
            0.0
        };

        let gpu = self.collect_gpu_metrics().await?;

        Ok(ResourceSnapshot {
            timestamp: chrono::Utc::now().to_rfc3339(),
            cpu: CpuMetrics {
                usage_millicores: total_cpu_usage,
                capacity_millicores: total_cpu_capacity,
                requests_millicores: 0, // TODO: aggregate from pods
                limits_millicores: 0,
                usage_percent: cpu_percent,
            },
            memory: MemoryMetrics {
                usage_bytes: total_mem_usage,
                capacity_bytes: total_mem_capacity,
                requests_bytes: 0,
                limits_bytes: 0,
                usage_percent: mem_percent,
            },
            gpu,
        })
    }

    async fn collect_node_metrics(&self) -> Result<Vec<NodeResourceStats>, MetricsError> {
        let nodes = self.list_nodes().await?;
        let mut stats = Vec::new();

        for node in nodes {
            let name = node
                .metadata
                .name
                .clone()
                .ok_or_else(|| MetricsError::Parse("Node has no name".into()))?;

            let instance_type = node
                .metadata
                .labels
                .as_ref()
                .and_then(|l| l.get("node.kubernetes.io/instance-type"))
                .cloned();

            let (cpu_capacity, mem_capacity) = if let Some(status) = &node.status {
                if let Some(alloc) = &status.allocatable {
                    let cpu = alloc
                        .get("cpu")
                        .map(|c| (parse_cpu_quantity(&c.0) * 1000.0) as i64)
                        .unwrap_or(0);
                    let mem = alloc
                        .get("memory")
                        .map(|m| parse_memory_quantity(&m.0))
                        .unwrap_or(0);
                    (cpu, mem)
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            };

            match self.get_node_stats(&name).await {
                Ok(raw) => {
                    let cpu_usage = (raw.cpu.usage_nano_cores / 1_000_000) as i64;
                    let mem_usage = raw.memory.usage_bytes as i64;

                    stats.push(NodeResourceStats {
                        node_name: name,
                        instance_type,
                        cpu: CpuMetrics {
                            usage_millicores: cpu_usage,
                            capacity_millicores: cpu_capacity,
                            requests_millicores: 0,
                            limits_millicores: 0,
                            usage_percent: if cpu_capacity > 0 {
                                (cpu_usage as f64 / cpu_capacity as f64) * 100.0
                            } else {
                                0.0
                            },
                        },
                        memory: MemoryMetrics {
                            usage_bytes: mem_usage,
                            capacity_bytes: mem_capacity,
                            requests_bytes: 0,
                            limits_bytes: 0,
                            usage_percent: if mem_capacity > 0 {
                                (mem_usage as f64 / mem_capacity as f64) * 100.0
                            } else {
                                0.0
                            },
                        },
                        cost_per_hour: None,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to get metrics for node {}: {}", name, e);
                }
            }
        }

        Ok(stats)
    }

    async fn collect_pod_metrics(
        &self,
        namespace: Option<&str>,
    ) -> Result<Vec<PodResourceStats>, MetricsError> {
        let pods = self.list_pods(namespace).await?;
        let mut stats = Vec::new();

        for pod in pods {
            let pod_name = pod
                .metadata
                .name
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let ns = pod
                .metadata
                .namespace
                .clone()
                .unwrap_or_else(|| "default".to_string());

            // Get requests/limits from pod spec
            let (cpu_requests, cpu_limits, mem_requests, mem_limits) =
                if let Some(spec) = &pod.spec {
                    let mut cpu_req = 0i64;
                    let mut cpu_lim = 0i64;
                    let mut mem_req = 0i64;
                    let mut mem_lim = 0i64;

                    for container in &spec.containers {
                        if let Some(resources) = &container.resources {
                            if let Some(requests) = &resources.requests {
                                if let Some(cpu) = requests.get("cpu") {
                                    cpu_req += (parse_cpu_quantity(&cpu.0) * 1000.0) as i64;
                                }
                                if let Some(mem) = requests.get("memory") {
                                    mem_req += parse_memory_quantity(&mem.0);
                                }
                            }
                            if let Some(limits) = &resources.limits {
                                if let Some(cpu) = limits.get("cpu") {
                                    cpu_lim += (parse_cpu_quantity(&cpu.0) * 1000.0) as i64;
                                }
                                if let Some(mem) = limits.get("memory") {
                                    mem_lim += parse_memory_quantity(&mem.0);
                                }
                            }
                        }
                    }

                    (cpu_req, cpu_lim, mem_req, mem_lim)
                } else {
                    (0, 0, 0, 0)
                };

            stats.push(PodResourceStats {
                pod_name,
                namespace: ns,
                cpu: CpuMetrics {
                    usage_millicores: 0, // TODO: get from metrics-server
                    capacity_millicores: cpu_limits,
                    requests_millicores: cpu_requests,
                    limits_millicores: cpu_limits,
                    usage_percent: 0.0,
                },
                memory: MemoryMetrics {
                    usage_bytes: 0,
                    capacity_bytes: mem_limits,
                    requests_bytes: mem_requests,
                    limits_bytes: mem_limits,
                    usage_percent: 0.0,
                },
                cost_per_hour: None,
            });
        }

        Ok(stats)
    }
}

/// Raw metrics from kubelet stats endpoint
#[derive(Deserialize)]
struct RawNodeMetrics {
    cpu: RawCpuMetrics,
    memory: RawMemoryMetrics,
}

#[derive(Deserialize)]
struct RawCpuMetrics {
    #[serde(rename = "usageNanoCores")]
    usage_nano_cores: usize,
}

#[derive(Deserialize)]
struct RawMemoryMetrics {
    #[serde(rename = "usageBytes")]
    usage_bytes: usize,
}

/// Parse CPU quantity string (e.g., "4", "500m", "2.5")
fn parse_cpu_quantity(s: &str) -> f64 {
    if s.ends_with('m') {
        s.trim_end_matches('m')
            .parse::<f64>()
            .unwrap_or(0.0)
            / 1000.0
    } else {
        s.parse::<f64>().unwrap_or(0.0)
    }
}

/// Parse memory quantity string (e.g., "1Gi", "512Mi", "1000Ki", "1000000")
fn parse_memory_quantity(s: &str) -> i64 {
    if s.ends_with("Gi") {
        s.trim_end_matches("Gi")
            .parse::<i64>()
            .unwrap_or(0)
            * 1024
            * 1024
            * 1024
    } else if s.ends_with("Mi") {
        s.trim_end_matches("Mi")
            .parse::<i64>()
            .unwrap_or(0)
            * 1024
            * 1024
    } else if s.ends_with("Ki") {
        s.trim_end_matches("Ki")
            .parse::<i64>()
            .unwrap_or(0)
            * 1024
    } else if s.ends_with('G') {
        s.trim_end_matches('G')
            .parse::<i64>()
            .unwrap_or(0)
            * 1000
            * 1000
            * 1000
    } else if s.ends_with('M') {
        s.trim_end_matches('M')
            .parse::<i64>()
            .unwrap_or(0)
            * 1000
            * 1000
    } else if s.ends_with('K') || s.ends_with('k') {
        s.trim_end_matches(['K', 'k'])
            .parse::<i64>()
            .unwrap_or(0)
            * 1000
    } else {
        s.parse::<i64>().unwrap_or(0)
    }
}
