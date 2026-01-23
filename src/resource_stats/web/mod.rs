//! Web UI module using Axum

use std::sync::Arc;
use std::time::Duration;
use std::convert::Infallible;

use axum::{
    extract::State,
    http::StatusCode,
    response::{
        Html, IntoResponse,
        sse::{Event, Sse},
    },
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use crate::resource_stats::types::resource_stats::ResourceStatsStatus;
use crate::resource_stats::UiState;

/// Application state for web handlers
#[derive(Clone)]
pub struct AppState {
    pub ui_state: Arc<UiState>,
}

/// Request body for triggering collection
#[derive(Debug, Deserialize)]
pub struct CollectRequest {
    /// Optional CRD name to save metrics to
    #[serde(default)]
    pub save_to_crd: Option<String>,
    /// Namespace for the CRD (default: default)
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

fn default_namespace() -> String {
    "default".to_string()
}

/// Response from collection trigger
#[derive(Debug, Serialize)]
pub struct CollectResponse {
    pub success: bool,
    pub message: String,
    pub stats: Option<ResourceStatsStatus>,
}

/// Create the Axum router with all routes
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // HTML dashboard
        .route("/", get(dashboard_handler))
        // API endpoints
        .route("/api/v1/stats/cluster", get(cluster_stats_handler))
        .route("/api/v1/stats/nodes", get(nodes_stats_handler))
        .route("/api/v1/health", get(health_handler))
        // SSE endpoint for real-time updates
        .route("/api/v1/stats/stream", get(sse_handler))
        // Webhook endpoint for on-demand collection
        .route("/api/v1/collect", post(collect_webhook_handler))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Dashboard HTML page
async fn dashboard_handler() -> impl IntoResponse {
    Html(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Resource Stats Dashboard</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <style>
        .progress-bar { transition: width 0.5s ease-in-out; }
        .fade-update { animation: fadeUpdate 0.3s ease-in-out; }
        @keyframes fadeUpdate {
            0% { opacity: 0.7; }
            100% { opacity: 1; }
        }
        .pulse-dot {
            animation: pulse 2s infinite;
        }
        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.5; }
        }
    </style>
</head>
<body class="bg-gray-900 text-white min-h-screen p-8">
    <div class="max-w-7xl mx-auto">
        <div class="flex items-center justify-between mb-8">
            <h1 class="text-3xl font-bold">Cluster Resource Dashboard</h1>
            <div class="flex items-center gap-2 text-sm text-gray-400">
                <span class="pulse-dot w-2 h-2 bg-green-500 rounded-full"></span>
                <span id="connection-status">Connecting...</span>
                <span id="last-update" class="ml-4"></span>
            </div>
        </div>

        <!-- Cluster Summary Cards -->
        <div class="grid grid-cols-1 md:grid-cols-3 gap-6 mb-8">
            <div class="bg-gray-800 rounded-xl p-6 shadow-lg">
                <div class="flex items-center justify-between mb-4">
                    <h2 class="text-lg font-semibold text-gray-400">CPU Usage</h2>
                    <svg class="w-6 h-6 text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2zM9 9h6v6H9V9z"/>
                    </svg>
                </div>
                <p class="text-4xl font-bold text-blue-400 mb-2" id="cpu-percent">--%</p>
                <div class="w-full bg-gray-700 rounded-full h-2 mb-2">
                    <div class="progress-bar bg-blue-500 h-2 rounded-full" id="cpu-bar" style="width: 0%"></div>
                </div>
                <p class="text-sm text-gray-500" id="cpu-detail">-- / -- cores</p>
            </div>

            <div class="bg-gray-800 rounded-xl p-6 shadow-lg">
                <div class="flex items-center justify-between mb-4">
                    <h2 class="text-lg font-semibold text-gray-400">Memory Usage</h2>
                    <svg class="w-6 h-6 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"/>
                    </svg>
                </div>
                <p class="text-4xl font-bold text-green-400 mb-2" id="mem-percent">--%</p>
                <div class="w-full bg-gray-700 rounded-full h-2 mb-2">
                    <div class="progress-bar bg-green-500 h-2 rounded-full" id="mem-bar" style="width: 0%"></div>
                </div>
                <p class="text-sm text-gray-500" id="mem-detail">-- / -- GB</p>
            </div>

            <div class="bg-gray-800 rounded-xl p-6 shadow-lg">
                <div class="flex items-center justify-between mb-4">
                    <h2 class="text-lg font-semibold text-gray-400">Nodes</h2>
                    <svg class="w-6 h-6 text-purple-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2"/>
                    </svg>
                </div>
                <p class="text-4xl font-bold text-purple-400 mb-2" id="node-count">--</p>
                <p class="text-sm text-gray-500" id="node-detail">Active nodes</p>
            </div>
        </div>

        <!-- Node Table -->
        <div class="bg-gray-800 rounded-xl p-6 shadow-lg mb-8">
            <h2 class="text-xl font-semibold text-gray-300 mb-4">Node Resources</h2>
            <div class="overflow-x-auto">
                <table class="w-full text-left">
                    <thead>
                        <tr class="text-gray-400 border-b border-gray-700">
                            <th class="pb-3 pr-4">Node</th>
                            <th class="pb-3 px-4 text-right">CPU Used</th>
                            <th class="pb-3 px-4 w-48">CPU %</th>
                            <th class="pb-3 px-4 text-right">Memory Used</th>
                            <th class="pb-3 px-4 w-48">Memory %</th>
                        </tr>
                    </thead>
                    <tbody id="node-table">
                        <tr><td colspan="5" class="py-4 text-gray-500 text-center">Waiting for data...</td></tr>
                    </tbody>
                </table>
            </div>
        </div>

        <!-- Raw Stats (collapsible) -->
        <details class="bg-gray-800 rounded-xl shadow-lg">
            <summary class="p-6 cursor-pointer text-gray-400 hover:text-gray-300">
                Raw JSON Data
            </summary>
            <pre id="stats" class="p-6 pt-0 text-xs text-gray-500 overflow-auto max-h-96">Waiting for data...</pre>
        </details>
    </div>

    <script>
        function formatBytes(bytes) {
            if (bytes === 0) return '0 B';
            const k = 1024;
            const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
            const i = Math.floor(Math.log(Math.abs(bytes)) / Math.log(k));
            return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
        }

        function getBarColor(percent) {
            if (percent >= 90) return 'bg-red-500';
            if (percent >= 75) return 'bg-yellow-500';
            return '';
        }

        function updateDashboard(data) {
            if (!data) return;

            // Update connection status
            document.getElementById('connection-status').textContent = 'Live';
            document.getElementById('last-update').textContent = 'Updated: ' + new Date().toLocaleTimeString();

            const current = data.current;
            if (current) {
                // CPU
                const cpuPercent = current.cpu.usage_percent.toFixed(1);
                document.getElementById('cpu-percent').textContent = cpuPercent + '%';
                document.getElementById('cpu-bar').style.width = cpuPercent + '%';
                document.getElementById('cpu-bar').className = 'progress-bar h-2 rounded-full bg-blue-500 ' + getBarColor(current.cpu.usage_percent);
                document.getElementById('cpu-detail').textContent =
                    (current.cpu.usage_millicores / 1000).toFixed(1) + ' / ' +
                    (current.cpu.capacity_millicores / 1000).toFixed(1) + ' cores';

                // Memory
                const memPercent = current.memory.usage_percent.toFixed(1);
                document.getElementById('mem-percent').textContent = memPercent + '%';
                document.getElementById('mem-bar').style.width = memPercent + '%';
                document.getElementById('mem-bar').className = 'progress-bar h-2 rounded-full bg-green-500 ' + getBarColor(current.memory.usage_percent);
                document.getElementById('mem-detail').textContent =
                    formatBytes(current.memory.usage_bytes) + ' / ' +
                    formatBytes(current.memory.capacity_bytes);
            }

            // Node count
            const nodes = data.node_stats || [];
            document.getElementById('node-count').textContent = nodes.length;
            document.getElementById('node-detail').textContent = nodes.length === 1 ? '1 node' : nodes.length + ' nodes';

            // Node table
            const tbody = document.getElementById('node-table');
            if (nodes.length === 0) {
                tbody.innerHTML = '<tr><td colspan="5" class="py-4 text-gray-500 text-center">No node data available</td></tr>';
            } else {
                tbody.innerHTML = nodes.map(node => {
                    const cpuPct = node.cpu.usage_percent.toFixed(1);
                    const memPct = node.memory.usage_percent.toFixed(1);
                    return `
                        <tr class="border-b border-gray-700/50 hover:bg-gray-700/30">
                            <td class="py-3 pr-4 font-medium text-gray-200">${node.node_name}</td>
                            <td class="py-3 px-4 text-right text-gray-400">${(node.cpu.usage_millicores / 1000).toFixed(2)} cores</td>
                            <td class="py-3 px-4">
                                <div class="flex items-center gap-2">
                                    <div class="flex-1 bg-gray-700 rounded-full h-2">
                                        <div class="progress-bar bg-blue-500 ${getBarColor(node.cpu.usage_percent)} h-2 rounded-full" style="width: ${cpuPct}%"></div>
                                    </div>
                                    <span class="text-sm text-gray-400 w-12 text-right">${cpuPct}%</span>
                                </div>
                            </td>
                            <td class="py-3 px-4 text-right text-gray-400">${formatBytes(node.memory.usage_bytes)}</td>
                            <td class="py-3 px-4">
                                <div class="flex items-center gap-2">
                                    <div class="flex-1 bg-gray-700 rounded-full h-2">
                                        <div class="progress-bar bg-green-500 ${getBarColor(node.memory.usage_percent)} h-2 rounded-full" style="width: ${memPct}%"></div>
                                    </div>
                                    <span class="text-sm text-gray-400 w-12 text-right">${memPct}%</span>
                                </div>
                            </td>
                        </tr>
                    `;
                }).join('');
            }

            // Raw stats
            document.getElementById('stats').textContent = JSON.stringify(data, null, 2);
        }

        function connectSSE() {
            const statusEl = document.getElementById('connection-status');
            statusEl.textContent = 'Connecting...';

            const evtSource = new EventSource('/api/v1/stats/stream');

            evtSource.onopen = () => {
                statusEl.textContent = 'Connected';
            };

            evtSource.addEventListener('stats', (event) => {
                try {
                    const data = JSON.parse(event.data);
                    updateDashboard(data);
                } catch (e) {
                    console.error('Failed to parse SSE data:', e);
                }
            });

            evtSource.onerror = () => {
                statusEl.textContent = 'Reconnecting...';
                evtSource.close();
                setTimeout(connectSSE, 3000);
            };
        }

        // Start SSE connection
        connectSSE();
    </script>
</body>
</html>"#,
    )
}

/// Cluster stats JSON endpoint
async fn cluster_stats_handler(
    State(state): State<AppState>,
) -> Json<Option<ResourceStatsStatus>> {
    let stats = state.ui_state.cluster_stats.read().await;
    Json(stats.clone())
}

/// Node stats JSON endpoint
async fn nodes_stats_handler(
    State(state): State<AppState>,
) -> Json<Vec<crate::resource_stats::types::resource_stats::NodeResourceStats>> {
    let stats = state.ui_state.cluster_stats.read().await;
    if let Some(s) = stats.as_ref() {
        Json(s.node_stats.clone())
    } else {
        Json(vec![])
    }
}

/// Health check endpoint
async fn health_handler() -> &'static str {
    "OK"
}

/// SSE handler for real-time stats streaming
async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let ui_state = state.ui_state.clone();

    let stream = async_stream::stream! {
        loop {
            // Wait before sending update
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Read current stats and serialize
            let data = {
                let stats = ui_state.cluster_stats.read().await;
                serde_json::to_string(&*stats).unwrap_or_else(|_| "null".to_string())
            };

            let event = Event::default()
                .event("stats")
                .data(data);

            yield Ok(event);
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

/// Webhook handler for on-demand collection
async fn collect_webhook_handler(
    State(state): State<AppState>,
    Json(request): Json<CollectRequest>,
) -> impl IntoResponse {
    use chrono::Utc;
    use kube::api::{Patch, PatchParams, PostParams};
    use kube::{Api, Client};

    use crate::resource_stats::metrics::{node::NodeMetricsCollector, MetricsCollector};
    use crate::resource_stats::types::resource_stats::{
        ResourceStats, ResourceStatsSpec, StatsPhase, StatsScope,
    };

    tracing::info!("Webhook triggered: collecting metrics");

    // Create Kubernetes client
    let client = match Client::try_default().await {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CollectResponse {
                    success: false,
                    message: format!("Failed to create Kubernetes client: {}", e),
                    stats: None,
                }),
            );
        }
    };

    let collector = NodeMetricsCollector::new(client.clone());

    // Collect metrics
    let snapshot = match collector.collect_snapshot().await {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CollectResponse {
                    success: false,
                    message: format!("Failed to collect snapshot: {}", e),
                    stats: None,
                }),
            );
        }
    };

    let nodes = match collector.collect_node_metrics().await {
        Ok(n) => n,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CollectResponse {
                    success: false,
                    message: format!("Failed to collect node metrics: {}", e),
                    stats: None,
                }),
            );
        }
    };

    // Build status
    let status = ResourceStatsStatus {
        phase: StatsPhase::Ready,
        current: Some(snapshot),
        cost_summary: None,
        node_stats: nodes,
        pod_stats: Vec::new(),
        gpu_stats: Vec::new(),
        history: Vec::new(),
        last_collection_time: Some(Utc::now().to_rfc3339()),
        conditions: Vec::new(),
        observed_generation: None,
    };

    // Update internal state
    {
        let mut stats_lock = state.ui_state.cluster_stats.write().await;
        *stats_lock = Some(status.clone());
    }

    // Save to CRD if requested
    if let Some(ref crd_name) = request.save_to_crd {
        let api: Api<ResourceStats> = Api::namespaced(client, &request.namespace);

        // Check if CRD exists
        let exists = api.get(crd_name).await.is_ok();

        if !exists {
            // Create the CRD
            let stats_crd = ResourceStats::new(crd_name, ResourceStatsSpec {
                scope: StatsScope::Cluster,
                target_ref: None,
                selector: None,
                interval: "1m".to_string(),
                retention: "24h".to_string(),
                cost_config_ref: None,
                collect_gpu: false,
            });

            if let Err(e) = api.create(&PostParams::default(), &stats_crd).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(CollectResponse {
                        success: false,
                        message: format!("Failed to create CRD: {}", e),
                        stats: Some(status),
                    }),
                );
            }
            tracing::info!("Created ResourceStats CRD: {}/{}", request.namespace, crd_name);
        }

        // Update the status
        let patch = serde_json::json!({ "status": status });
        if let Err(e) = api
            .patch_status(
                crd_name,
                &PatchParams::apply("resource-stats-webhook"),
                &Patch::Merge(&patch),
            )
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CollectResponse {
                    success: false,
                    message: format!("Failed to update CRD status: {}", e),
                    stats: Some(status),
                }),
            );
        }

        tracing::info!("Updated ResourceStats CRD: {}/{}", request.namespace, crd_name);
    }

    let message = match &request.save_to_crd {
        Some(name) => format!("Metrics collected and saved to {}/{}", request.namespace, name),
        None => "Metrics collected (not saved to CRD)".to_string(),
    };

    (
        StatusCode::OK,
        Json(CollectResponse {
            success: true,
            message,
            stats: Some(status),
        }),
    )
}

/// Start the web server
pub async fn start_server(state: AppState, addr: &str) -> Result<(), std::io::Error> {
    let router = create_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Web server listening on {}", addr);
    axum::serve(listener, router).await?;
    Ok(())
}
