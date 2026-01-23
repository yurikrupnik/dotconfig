//! Cluster Dashboard Binary
//!
//! A terminal-based dashboard for cluster management that shows:
//! - Cluster information and health
//! - Dependencies and their status (operators, CRDs)
//! - Applications and their state
//! - Security vulnerabilities and best practices
//! - FinOps cost analysis
//! - Port forwards for local development

use clap::Parser;
use dotconfig::cluster_dashboard::Dashboard;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "cluster-dashboard")]
#[command(about = "Terminal UI dashboard for Kubernetes cluster management")]
#[command(version)]
struct Args {
    /// Kubernetes context to use (defaults to current context)
    #[arg(short, long)]
    context: Option<String>,

    /// Namespace to focus on (defaults to all namespaces)
    #[arg(short, long)]
    namespace: Option<String>,

    /// Refresh interval in seconds
    #[arg(short, long, default_value = "30")]
    refresh: u64,

    /// Path to kubeconfig file
    #[arg(long, env = "KUBECONFIG")]
    kubeconfig: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing (to file to avoid messing up TUI)
    let file_appender = tracing_subscriber::fmt::writer::MakeWriterExt::with_max_level(
        std::io::stderr,
        tracing::Level::WARN,
    );

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(file_appender)
        .with_ansi(false)
        .init();

    let args = Args::parse();

    info!(
        context = ?args.context,
        namespace = ?args.namespace,
        refresh = args.refresh,
        "Starting Cluster Dashboard"
    );

    // Create and run a dashboard
    let mut dashboard = Dashboard::new().await?;
    dashboard.run().await?;

    Ok(())
}
