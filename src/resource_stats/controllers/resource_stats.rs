//! ResourceStats controller - collects and reports resource metrics

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use tracing::{error, info, warn};

use crate::resource_stats::types::resource_stats::{
    HistoricalSample, ResourceStats, ResourceStatsStatus, StatsPhase, StatsScope,
};
use crate::resource_stats::ResourceStatsContext;
use crate::resource_stats::ResourceStatsError;

/// Maximum historical samples to keep
const MAX_HISTORY_SAMPLES: usize = 60;

/// Reconciles ResourceStats resources
pub async fn reconcile(
    stats: Arc<ResourceStats>,
    ctx: Arc<ResourceStatsContext>,
) -> Result<Action, ResourceStatsError> {
    let name = stats.name_any();
    let namespace = stats.namespace().unwrap_or_else(|| "default".to_string());

    info!("Reconciling ResourceStats {}/{}", namespace, name);

    let api: Api<ResourceStats> = Api::namespaced(ctx.client.clone(), &namespace);

    // Update status to Collecting
    update_phase(&api, &name, StatsPhase::Collecting).await?;

    // Collect metrics based on scope
    let result = match &stats.spec.scope {
        StatsScope::Cluster => collect_cluster_metrics(&stats, &ctx).await,
        StatsScope::Node => collect_node_metrics(&stats, &ctx).await,
        StatsScope::Namespace => collect_namespace_metrics(&stats, &ctx).await,
        StatsScope::Deployment => collect_deployment_metrics(&stats, &ctx).await,
        StatsScope::Pod => collect_pod_metrics(&stats, &ctx).await,
    };

    match result {
        Ok(mut new_status) => {
            // Preserve and append to history
            if let Some(current) = &new_status.current {
                let sample = HistoricalSample {
                    timestamp: current.timestamp.clone(),
                    cpu_usage_percent: current.cpu.usage_percent,
                    memory_usage_percent: current.memory.usage_percent,
                    gpu_usage_percent: current.gpu.as_ref().map(|g| g.utilization_percent),
                    cost_per_hour: new_status
                        .cost_summary
                        .as_ref()
                        .map(|c| c.total_per_hour.clone())
                        .unwrap_or_else(|| "$0.00".to_string()),
                };

                // Get existing history and append
                let existing = api.get_status(&name).await.ok();
                let mut history: Vec<HistoricalSample> = existing
                    .and_then(|s| s.status)
                    .map(|s| s.history)
                    .unwrap_or_default();

                history.push(sample);

                // Trim to max samples
                if history.len() > MAX_HISTORY_SAMPLES {
                    history = history.split_off(history.len() - MAX_HISTORY_SAMPLES);
                }

                new_status.history = history;
            }

            new_status.phase = StatsPhase::Ready;
            new_status.last_collection_time = Some(Utc::now().to_rfc3339());

            update_status(&api, &name, new_status).await?;
            info!("ResourceStats {}/{} is Ready", namespace, name);
        }
        Err(e) => {
            update_phase(&api, &name, StatsPhase::Failed).await?;
            error!("ResourceStats {}/{} failed: {}", namespace, name, e);
            return Err(e);
        }
    }

    // Requeue based on collection interval
    let interval_secs = parse_duration(&stats.spec.interval).unwrap_or(60);
    Ok(Action::requeue(Duration::from_secs(interval_secs)))
}

/// Error policy for ResourceStats reconciliation
pub fn error_policy(
    _stats: Arc<ResourceStats>,
    error: &ResourceStatsError,
    _ctx: Arc<ResourceStatsContext>,
) -> Action {
    error!("ResourceStats reconcile error: {}", error);
    Action::requeue(Duration::from_secs(30))
}

/// Collect cluster-wide metrics
async fn collect_cluster_metrics(
    stats: &ResourceStats,
    ctx: &ResourceStatsContext,
) -> Result<ResourceStatsStatus, ResourceStatsError> {
    info!("Collecting cluster-wide metrics");

    let snapshot = ctx.metrics_collector.collect_snapshot().await?;
    let node_stats = ctx.metrics_collector.collect_node_metrics().await?;

    // Calculate costs
    let rates = ctx.cost_calculator.get_rates(None).await?;
    let cost_summary = ctx.cost_calculator.calculate_cost(&snapshot, &rates);

    // Collect GPU metrics if enabled
    let gpu_stats = if stats.spec.collect_gpu {
        let mut all_gpu: Vec<crate::resource_stats::types::resource_stats::GpuResourceStats> =
            Vec::new();
        for collector in &ctx.gpu_collectors {
            if collector.is_available() {
                match collector.collect().await {
                    Ok(gpus) => all_gpu.extend(gpus),
                    Err(e) => warn!("GPU collector {} failed: {}", collector.vendor(), e),
                }
            }
        }
        all_gpu
    } else {
        Vec::new()
    };

    Ok(ResourceStatsStatus {
        phase: StatsPhase::Collecting,
        current: Some(snapshot),
        cost_summary: Some(cost_summary),
        node_stats,
        pod_stats: Vec::new(),
        gpu_stats,
        history: Vec::new(),
        last_collection_time: None,
        conditions: Vec::new(),
        observed_generation: None,
    })
}

/// Collect metrics for a specific node
async fn collect_node_metrics(
    stats: &ResourceStats,
    ctx: &ResourceStatsContext,
) -> Result<ResourceStatsStatus, ResourceStatsError> {
    let target = stats.spec.target_ref.as_ref().ok_or_else(|| {
        ResourceStatsError::Config("Node scope requires targetRef".into())
    })?;

    info!("Collecting metrics for node {}", target.name);

    let all_nodes = ctx.metrics_collector.collect_node_metrics().await?;
    let node_stats: Vec<_> = all_nodes
        .into_iter()
        .filter(|n| n.node_name == target.name)
        .collect();

    if node_stats.is_empty() {
        return Err(ResourceStatsError::Config(format!(
            "Node {} not found",
            target.name
        )));
    }

    let node = &node_stats[0];

    // Create a snapshot from the node metrics
    let snapshot = crate::resource_stats::types::resource_stats::ResourceSnapshot {
        timestamp: Utc::now().to_rfc3339(),
        cpu: node.cpu.clone(),
        memory: node.memory.clone(),
        gpu: None,
    };

    let rates = ctx.cost_calculator.get_rates(node.instance_type.as_deref()).await?;
    let cost_summary = ctx.cost_calculator.calculate_cost(&snapshot, &rates);

    Ok(ResourceStatsStatus {
        phase: StatsPhase::Collecting,
        current: Some(snapshot),
        cost_summary: Some(cost_summary),
        node_stats,
        pod_stats: Vec::new(),
        gpu_stats: Vec::new(),
        history: Vec::new(),
        last_collection_time: None,
        conditions: Vec::new(),
        observed_generation: None,
    })
}

/// Collect metrics for a namespace
async fn collect_namespace_metrics(
    stats: &ResourceStats,
    ctx: &ResourceStatsContext,
) -> Result<ResourceStatsStatus, ResourceStatsError> {
    let target = stats.spec.target_ref.as_ref().ok_or_else(|| {
        ResourceStatsError::Config("Namespace scope requires targetRef".into())
    })?;

    info!("Collecting metrics for namespace {}", target.name);

    let pod_stats = ctx
        .metrics_collector
        .collect_pod_metrics(Some(&target.name))
        .await?;

    // Aggregate pod metrics
    let total_cpu_usage: i64 = pod_stats.iter().map(|p| p.cpu.usage_millicores).sum();
    let total_cpu_requests: i64 = pod_stats.iter().map(|p| p.cpu.requests_millicores).sum();
    let total_mem_usage: i64 = pod_stats.iter().map(|p| p.memory.usage_bytes).sum();
    let total_mem_requests: i64 = pod_stats.iter().map(|p| p.memory.requests_bytes).sum();

    let snapshot = crate::resource_stats::types::resource_stats::ResourceSnapshot {
        timestamp: Utc::now().to_rfc3339(),
        cpu: crate::resource_stats::types::resource_stats::CpuMetrics {
            usage_millicores: total_cpu_usage,
            capacity_millicores: total_cpu_requests,
            requests_millicores: total_cpu_requests,
            limits_millicores: 0,
            usage_percent: if total_cpu_requests > 0 {
                (total_cpu_usage as f64 / total_cpu_requests as f64) * 100.0
            } else {
                0.0
            },
        },
        memory: crate::resource_stats::types::resource_stats::MemoryMetrics {
            usage_bytes: total_mem_usage,
            capacity_bytes: total_mem_requests,
            requests_bytes: total_mem_requests,
            limits_bytes: 0,
            usage_percent: if total_mem_requests > 0 {
                (total_mem_usage as f64 / total_mem_requests as f64) * 100.0
            } else {
                0.0
            },
        },
        gpu: None,
    };

    let rates = ctx.cost_calculator.get_rates(None).await?;
    let cost_summary = ctx.cost_calculator.calculate_cost(&snapshot, &rates);

    Ok(ResourceStatsStatus {
        phase: StatsPhase::Collecting,
        current: Some(snapshot),
        cost_summary: Some(cost_summary),
        node_stats: Vec::new(),
        pod_stats,
        gpu_stats: Vec::new(),
        history: Vec::new(),
        last_collection_time: None,
        conditions: Vec::new(),
        observed_generation: None,
    })
}

/// Collect metrics for a deployment
async fn collect_deployment_metrics(
    stats: &ResourceStats,
    ctx: &ResourceStatsContext,
) -> Result<ResourceStatsStatus, ResourceStatsError> {
    let target = stats.spec.target_ref.as_ref().ok_or_else(|| {
        ResourceStatsError::Config("Deployment scope requires targetRef".into())
    })?;

    let ns = target.namespace.as_deref().unwrap_or("default");
    info!("Collecting metrics for deployment {}/{}", ns, target.name);

    // For deployment scope, we'd need to find pods by label selector
    // For now, collect all pods in namespace and filter by owner reference
    let all_pods = ctx.metrics_collector.collect_pod_metrics(Some(ns)).await?;

    // TODO: Filter by deployment owner reference
    let pod_stats = all_pods;

    let snapshot = crate::resource_stats::types::resource_stats::ResourceSnapshot {
        timestamp: Utc::now().to_rfc3339(),
        cpu: crate::resource_stats::types::resource_stats::CpuMetrics {
            usage_millicores: pod_stats.iter().map(|p| p.cpu.usage_millicores).sum(),
            capacity_millicores: 0,
            requests_millicores: pod_stats.iter().map(|p| p.cpu.requests_millicores).sum(),
            limits_millicores: 0,
            usage_percent: 0.0,
        },
        memory: crate::resource_stats::types::resource_stats::MemoryMetrics {
            usage_bytes: pod_stats.iter().map(|p| p.memory.usage_bytes).sum(),
            capacity_bytes: 0,
            requests_bytes: pod_stats.iter().map(|p| p.memory.requests_bytes).sum(),
            limits_bytes: 0,
            usage_percent: 0.0,
        },
        gpu: None,
    };

    let rates = ctx.cost_calculator.get_rates(None).await?;
    let cost_summary = ctx.cost_calculator.calculate_cost(&snapshot, &rates);

    Ok(ResourceStatsStatus {
        phase: StatsPhase::Collecting,
        current: Some(snapshot),
        cost_summary: Some(cost_summary),
        node_stats: Vec::new(),
        pod_stats,
        gpu_stats: Vec::new(),
        history: Vec::new(),
        last_collection_time: None,
        conditions: Vec::new(),
        observed_generation: None,
    })
}

/// Collect metrics for a specific pod
async fn collect_pod_metrics(
    stats: &ResourceStats,
    ctx: &ResourceStatsContext,
) -> Result<ResourceStatsStatus, ResourceStatsError> {
    let target = stats.spec.target_ref.as_ref().ok_or_else(|| {
        ResourceStatsError::Config("Pod scope requires targetRef".into())
    })?;

    let ns = target.namespace.as_deref().unwrap_or("default");
    info!("Collecting metrics for pod {}/{}", ns, target.name);

    let all_pods = ctx.metrics_collector.collect_pod_metrics(Some(ns)).await?;
    let pod_stats: Vec<_> = all_pods
        .into_iter()
        .filter(|p| p.pod_name == target.name)
        .collect();

    if pod_stats.is_empty() {
        return Err(ResourceStatsError::Config(format!(
            "Pod {}/{} not found",
            ns, target.name
        )));
    }

    let pod = &pod_stats[0];
    let snapshot = crate::resource_stats::types::resource_stats::ResourceSnapshot {
        timestamp: Utc::now().to_rfc3339(),
        cpu: pod.cpu.clone(),
        memory: pod.memory.clone(),
        gpu: None,
    };

    let rates = ctx.cost_calculator.get_rates(None).await?;
    let cost_summary = ctx.cost_calculator.calculate_cost(&snapshot, &rates);

    Ok(ResourceStatsStatus {
        phase: StatsPhase::Collecting,
        current: Some(snapshot),
        cost_summary: Some(cost_summary),
        node_stats: Vec::new(),
        pod_stats,
        gpu_stats: Vec::new(),
        history: Vec::new(),
        last_collection_time: None,
        conditions: Vec::new(),
        observed_generation: None,
    })
}

/// Update ResourceStats phase
async fn update_phase(
    api: &Api<ResourceStats>,
    name: &str,
    phase: StatsPhase,
) -> Result<(), ResourceStatsError> {
    let patch = serde_json::json!({
        "status": {
            "phase": phase,
            "lastCollectionTime": Utc::now().to_rfc3339()
        }
    });
    api.patch_status(name, &PatchParams::apply("resource-stats-operator"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

/// Update full ResourceStats status
async fn update_status(
    api: &Api<ResourceStats>,
    name: &str,
    status: ResourceStatsStatus,
) -> Result<(), ResourceStatsError> {
    let patch = serde_json::json!({ "status": status });
    api.patch_status(name, &PatchParams::apply("resource-stats-operator"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

/// Parse duration string to seconds
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
