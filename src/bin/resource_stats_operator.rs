//! Resource Stats Operator binary
//!
//! Collects CPU/memory/GPU metrics, calculates costs, and exposes via web/TUI.

use std::sync::Arc;

use clap::{Parser, Subcommand};
use kube::Client;
use tracing::info;

use dotconfig::resource_stats::{
    metrics::{node::NodeMetricsCollector, MetricsCollector},
    UiState,
};

#[derive(Parser)]
#[command(name = "resource-stats-operator")]
#[command(about = "Kubernetes Resource Stats Operator with cost tracking")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Web server bind address
    #[arg(long, default_value = "0.0.0.0:8080")]
    web_addr: String,

    /// Enable web UI
    #[arg(long, default_value = "true")]
    enable_web: bool,

    /// Run in TUI mode
    #[arg(long)]
    tui: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the full operator with CRD controllers
    Run {
        /// Also start web server
        #[arg(long)]
        with_web: bool,
        /// Web server bind address
        #[arg(long, default_value = "0.0.0.0:8080")]
        addr: String,
    },
    /// Run the operator with web UI only (no CRD controllers)
    Serve {
        /// Bind address for web server
        #[arg(long, default_value = "0.0.0.0:8080")]
        addr: String,
    },
    /// Run terminal UI
    Tui,
    /// Collect and print metrics once
    Collect {
        /// Output format (json, table)
        #[arg(long, default_value = "table")]
        format: String,

        /// Save metrics to a ResourceStats CRD (creates or updates)
        #[arg(long)]
        save_to_crd: Option<String>,

        /// Namespace for the CRD (default: default)
        #[arg(long, default_value = "default")]
        namespace: String,
    },
    /// Generate CRD manifests
    Crds,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing (output to stderr so stdout is clean for JSON/piping)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("resource_stats=info".parse()?)
                .add_directive("kube=warn".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Run { with_web, addr }) => {
            run_operator(with_web, &addr).await?;
        }
        Some(Commands::Serve { addr }) => {
            run_server(&addr).await?;
        }
        Some(Commands::Tui) => {
            run_tui().await?;
        }
        Some(Commands::Collect { format, save_to_crd, namespace }) => {
            collect_once(&format, save_to_crd.as_deref(), &namespace).await?;
        }
        Some(Commands::Crds) => {
            print_crds();
        }
        None => {
            // Default: run based on flags
            if cli.tui {
                run_tui().await?;
            } else if cli.enable_web {
                run_server(&cli.web_addr).await?;
            } else {
                collect_once("table", None, "default").await?;
            }
        }
    }

    Ok(())
}

/// Run the full operator with CRD controllers
async fn run_operator(with_web: bool, addr: &str) -> anyhow::Result<()> {
    use futures::StreamExt;
    use kube::runtime::Controller;
    use kube::Api;

    use dotconfig::resource_stats::{
        cache::MetricsCache,
        controllers::{
            cost_config_error_policy, reconcile_cost_config,
            resource_stats_error_policy, reconcile_resource_stats,
        },
        cost::static_pricing::StaticPricingCalculator,
        types::{CostConfig, ResourceStats},
        ResourceStatsContext,
    };

    info!("Starting Resource Stats Operator");

    let client = Client::try_default().await?;

    // Build context
    let metrics_collector = Arc::new(NodeMetricsCollector::new(client.clone()));
    let cost_calculator = Arc::new(StaticPricingCalculator::new());
    let cache = Arc::new(MetricsCache::new());
    let ui_state = Arc::new(UiState::new());

    let ctx = Arc::new(ResourceStatsContext {
        client: client.clone(),
        metrics_collector,
        cost_calculator,
        gpu_collectors: Vec::new(),
        cache,
        ui_state: ui_state.clone(),
    });

    // Setup controllers
    let cost_config_api: Api<CostConfig> = Api::all(client.clone());
    let resource_stats_api: Api<ResourceStats> = Api::all(client.clone());

    let cost_config_controller = Controller::new(cost_config_api, Default::default())
        .shutdown_on_signal()
        .run(reconcile_cost_config, cost_config_error_policy, ctx.clone())
        .for_each(|res| async move {
            match res {
                Ok(o) => info!("CostConfig reconciled: {:?}", o),
                Err(e) => tracing::error!("CostConfig reconcile error: {:?}", e),
            }
        });

    let resource_stats_controller = Controller::new(resource_stats_api, Default::default())
        .shutdown_on_signal()
        .run(reconcile_resource_stats, resource_stats_error_policy, ctx.clone())
        .for_each(|res| async move {
            match res {
                Ok(o) => info!("ResourceStats reconciled: {:?}", o),
                Err(e) => tracing::error!("ResourceStats reconcile error: {:?}", e),
            }
        });

    info!("Controllers started. Watching for CostConfig and ResourceStats resources...");

    if with_web {
        #[cfg(feature = "web-ui")]
        {
            use dotconfig::resource_stats::web::{start_server, AppState};

            let web_state = AppState { ui_state };
            let addr_owned = addr.to_string();

            tokio::select! {
                _ = cost_config_controller => { tracing::error!("CostConfig controller exited"); }
                _ = resource_stats_controller => { tracing::error!("ResourceStats controller exited"); }
                _ = start_server(web_state, &addr_owned) => { tracing::error!("Web server exited"); }
            }
        }
        #[cfg(not(feature = "web-ui"))]
        {
            anyhow::bail!("Web UI feature not enabled");
        }
    } else {
        tokio::select! {
            _ = cost_config_controller => { tracing::error!("CostConfig controller exited"); }
            _ = resource_stats_controller => { tracing::error!("ResourceStats controller exited"); }
        }
    }

    Ok(())
}

/// Run the web server
#[cfg(feature = "web-ui")]
async fn run_server(addr: &str) -> anyhow::Result<()> {
    use std::time::Duration;
    use dotconfig::resource_stats::web::{start_server, AppState};

    info!("Starting resource stats operator with web UI");

    let client = Client::try_default().await?;
    let ui_state = Arc::new(UiState::new());

    // Start metrics collection in background
    let metrics_collector = Arc::new(NodeMetricsCollector::new(client.clone()));
    let ui_state_clone = ui_state.clone();

    tokio::spawn(async move {
        loop {
            match metrics_collector.collect_snapshot().await {
                Ok(snapshot) => {
                    let mut stats = ui_state_clone.cluster_stats.write().await;
                    *stats = Some(dotconfig::resource_stats::types::resource_stats::ResourceStatsStatus {
                        phase: dotconfig::resource_stats::types::resource_stats::StatsPhase::Ready,
                        current: Some(snapshot),
                        ..Default::default()
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to collect metrics: {}", e);
                }
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });

    let state = AppState { ui_state };
    start_server(state, addr).await?;

    Ok(())
}

#[cfg(not(feature = "web-ui"))]
async fn run_server(_addr: &str) -> anyhow::Result<()> {
    anyhow::bail!("Web UI feature not enabled. Rebuild with --features web-ui");
}

/// Run the TUI
#[cfg(feature = "tui")]
async fn run_tui() -> anyhow::Result<()> {
    use dotconfig::resource_stats::tui::App;

    info!("Starting resource stats operator in TUI mode");

    let ui_state = Arc::new(UiState::new());
    let mut app = App::new(ui_state);
    app.run().await?;

    Ok(())
}

#[cfg(not(feature = "tui"))]
async fn run_tui() -> anyhow::Result<()> {
    anyhow::bail!("TUI feature not enabled. Rebuild with --features tui");
}

/// Collect metrics once and print
async fn collect_once(format: &str, save_to_crd: Option<&str>, namespace: &str) -> anyhow::Result<()> {
    use chrono::Utc;
    use kube::Api;
    use kube::api::{Patch, PatchParams, PostParams};
    use dotconfig::resource_stats::types::resource_stats::{
        ResourceStats, ResourceStatsSpec, ResourceStatsStatus, StatsPhase, StatsScope,
    };

    let client = Client::try_default().await?;
    let collector = NodeMetricsCollector::new(client.clone());

    info!("Collecting cluster metrics...");

    let snapshot = collector.collect_snapshot().await?;
    let nodes = collector.collect_node_metrics().await?;

    // Save to CRD if requested
    if let Some(crd_name) = save_to_crd {
        info!("Saving metrics to ResourceStats CRD: {}/{}", namespace, crd_name);

        let api: Api<ResourceStats> = Api::namespaced(client, namespace);

        // Check if CRD exists
        let exists = api.get(crd_name).await.is_ok();

        if !exists {
            // Create the CRD
            let stats = ResourceStats::new(crd_name, ResourceStatsSpec {
                scope: StatsScope::Cluster,
                target_ref: None,
                selector: None,
                interval: "1m".to_string(),
                retention: "24h".to_string(),
                cost_config_ref: None,
                collect_gpu: false,
            });

            api.create(&PostParams::default(), &stats).await?;
            info!("Created ResourceStats CRD: {}/{}", namespace, crd_name);
        }

        // Update the status with collected metrics
        let status = ResourceStatsStatus {
            phase: StatsPhase::Ready,
            current: Some(snapshot.clone()),
            cost_summary: None,
            node_stats: nodes.clone(),
            pod_stats: Vec::new(),
            gpu_stats: Vec::new(),
            history: Vec::new(),
            last_collection_time: Some(Utc::now().to_rfc3339()),
            conditions: Vec::new(),
            observed_generation: None,
        };

        let patch = serde_json::json!({ "status": status });
        api.patch_status(
            crd_name,
            &PatchParams::apply("resource-stats-collector"),
            &Patch::Merge(&patch),
        ).await?;

        info!("Updated ResourceStats status for: {}/{}", namespace, crd_name);
    }

    match format {
        "json" => {
            let output = serde_json::json!({
                "snapshot": snapshot,
                "nodes": nodes,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        _ => {
            // Table format
            println!("\nCluster Summary:");
            println!("  CPU:    {}m / {}m ({:.1}%)",
                snapshot.cpu.usage_millicores,
                snapshot.cpu.capacity_millicores,
                snapshot.cpu.usage_percent
            );
            println!("  Memory: {} / {} ({:.1}%)",
                format_bytes(snapshot.memory.usage_bytes),
                format_bytes(snapshot.memory.capacity_bytes),
                snapshot.memory.usage_percent
            );

            println!("\nNodes:");
            println!("{:<40} {:>12} {:>8} {:>12} {:>8}",
                "NAME", "CPU", "CPU%", "MEMORY", "MEM%"
            );
            println!("{}", "-".repeat(84));

            for node in nodes {
                println!("{:<40} {:>10}m {:>7.1}% {:>12} {:>7.1}%",
                    node.node_name,
                    node.cpu.usage_millicores,
                    node.cpu.usage_percent,
                    format_bytes(node.memory.usage_bytes),
                    node.memory.usage_percent
                );
            }
        }
    }

    Ok(())
}

/// Print CRD manifests
fn print_crds() {
    use kube::CustomResourceExt;
    use dotconfig::resource_stats::types::{CostConfig, ResourceStats};

    let cost_config_crd = CostConfig::crd();
    let resource_stats_crd = ResourceStats::crd();

    println!("---");
    println!("{}", serde_yaml::to_string(&cost_config_crd).unwrap());
    println!("---");
    println!("{}", serde_yaml::to_string(&resource_stats_crd).unwrap());
}

/// Format bytes to human readable
fn format_bytes(bytes: i64) -> String {
    const GIB: i64 = 1024 * 1024 * 1024;
    const MIB: i64 = 1024 * 1024;

    if bytes >= GIB {
        format!("{:.1}Gi", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.0}Mi", bytes as f64 / MIB as f64)
    } else {
        format!("{}B", bytes)
    }
}
