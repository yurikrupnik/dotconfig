use clap::Subcommand;
use crate::commands::RunCommand;
use crate::app::App;

#[derive(Subcommand)]
pub enum ShitAction {
    DoIt {
        #[arg(short, long)]
        name: Option<String>,
    },
    Project {
        #[arg(short, long)]
        name: Option<String>,
        #[arg(short, long)]
        dry_run: Option<bool>,
        #[arg(short = 's', long)]
        docker_runtime: Option<String>,
    },
}

#[async_trait::async_trait]
impl RunCommand for ShitAction {
    async fn run(&self, app: &App) -> anyhow::Result<()> {
        match self {
            ShitAction::DoIt { name } => {
                if app.ctx.dry_run {
                    tracing::info!("DRY-RUN: Would do it with {name:?}");
                } else {
                    tracing::info!("Doing it with {name:?}");
                }
            },
            ShitAction::Project { dry_run, docker_runtime, .. } => {
                let effective_dry_run = *dry_run.as_ref().unwrap_or(&false) || app.ctx.dry_run;
                if effective_dry_run {
                    tracing::info!("DRY-RUN: Would manage project");
                } else {
                    tracing::info!("Managing project with runtime: {docker_runtime:?}");
                }
            }
        }
        Ok(())
    }
}
