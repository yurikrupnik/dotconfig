mod actions;
mod app;
mod commands;
mod config;
mod context;
mod crates;
mod state;
mod traits;
mod utils;

use actions::{ClusterAction, CodeGraphAction, ComposeAction, DashboardAction, ShitAction};
use app::App;
use clap::{CommandFactory, Parser, Subcommand};
use commands::RunCommand;
use config::Config;
use context::{AppContext, OutputFormat};
use crates::tracing::init_tracing_with_level;
use state::AppState;

#[derive(Subcommand)]
pub enum Commands {
    /// Kubernetes cluster management and health checks
    Cluster {
        #[command(subcommand)]
        action: ClusterAction,
    },
    Compose {
        #[command(subcommand)]
        action: ComposeAction,
    },
    Shit {
        #[command(subcommand)]
        action: ShitAction,
    },
    Dashboard {
        #[command(subcommand)]
        action: DashboardAction,
    },
    CodeGraph {
        #[command(subcommand)]
        action: CodeGraphAction,
    },
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Parser)]
#[command(name = "dotconfig")]
#[command(about = "Local development utilities")]
#[command(version)]
#[command(long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, action = clap::ArgAction::Count)]
    #[arg(help = "Increase logging verbosity (-d for debug, -dd for trace)")]
    debug: u8,

    #[arg(short = 's', long)]
    #[arg(help = "Run in dry-run mode (don't execute destructive operations)")]
    dry_run: bool,

    #[arg(short, long, value_enum, default_value = "human")]
    #[arg(help = "Output format")]
    output: OutputFormat,

    #[arg(long)]
    #[arg(help = "Disable colored output")]
    no_color: bool,

    #[arg(long, env = "POSTGRES_URL")]
    #[arg(help = "Override Postgres URL")]
    pub postgres_url: Option<String>,

    #[arg(long, env = "REDIS_URL")]
    #[arg(help = "Override Redis URL")]
    pub redis_url: Option<String>,

    #[arg(long, env = "MONGO_URL")]
    #[arg(help = "Override Mongo URL")]
    pub mongo_url: Option<String>,

    #[arg(long, env = "DOTCONFIG_CONFIG")]
    #[arg(help = "Path to config file")]
    pub config: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Cli {
        command,
        debug,
        dry_run,
        output,
        no_color,
        postgres_url,
        redis_url,
        mongo_url,
        config,
    } = Cli::parse();

    let config = Config::load_or_default(config);

    let merged_config = config.merge_with_cli(postgres_url, redis_url, mongo_url);

    let ctx = AppContext::new(
        debug,
        dry_run,
        output,
        no_color,
        merged_config.database.postgres_url.clone(),
        merged_config.database.redis_url.clone(),
        merged_config.database.mongo_url.clone(),
    );

    init_tracing_with_level(ctx.tracing_level(), ctx.no_color);

    if ctx.dry_run {
        tracing::warn!("Running in DRY-RUN mode - no destructive operations will be executed");
    }

    let state = AppState::with_databases(
        merged_config.database.postgres_url,
        merged_config.database.redis_url,
        merged_config.database.mongo_url,
    )
    .await?;

    let app = App::new(ctx, state);

    match command {
        Commands::Completions { shell } => {
            generate_completions(shell);
            Ok(())
        }
        Commands::Cluster { action } => action.run(&app).await,
        Commands::Compose { action } => action.run(&app).await,
        Commands::Shit { action } => action.run(&app).await,
        Commands::Dashboard { action } => action.run(&app).await,
        Commands::CodeGraph { action } => action.run(&app).await,
    }
}

fn generate_completions(shell: clap_complete::Shell) {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
}

#[cfg(test)]
mod test {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }
}
