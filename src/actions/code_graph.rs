use clap::Subcommand;
use std::path::PathBuf;
use crate::commands::RunCommand;
use crate::app::App;
use crate::crates::code_graph::CodeGraphClient;

#[derive(Subcommand)]
pub enum CodeGraphAction {
    Init,
    Scan {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    Clear,
    Query {
        cypher: String,
    },
    Stats,
}

#[async_trait::async_trait]
impl RunCommand for CodeGraphAction {
    async fn run(&self, app: &App) -> anyhow::Result<()> {
        let client = CodeGraphClient::new(
            &app.state.neo4j_uri,
            &app.state.neo4j_username,
            &app.state.neo4j_password,
        )
        .await?;

        match self {
            CodeGraphAction::Init => {
                if app.ctx.dry_run {
                    tracing::info!("DRY-RUN: Would initialize knowledge graph schema");
                    return Ok(());
                }

                client.init().await?;
                tracing::info!("Knowledge graph initialized successfully");
            }

            CodeGraphAction::Scan { path } => {
                if app.ctx.dry_run {
                    tracing::info!("DRY-RUN: Would scan workspace at: {:?}", path);
                    return Ok(());
                }

                let workspace_root = std::fs::canonicalize(path)?;
                tracing::info!("Scanning workspace at: {}", workspace_root.display());

                let progress = client.scan_workspace(&workspace_root).await?;
                tracing::info!(
                    "Scan completed: {}/{} projects scanned",
                    progress.projects_scanned,
                    progress.total_projects
                );
            }

            CodeGraphAction::Clear => {
                if app.ctx.dry_run {
                    tracing::info!("DRY-RUN: Would clear all nodes from the graph");
                    return Ok(());
                }

                let (nodes, _) = client.clear().await?;
                tracing::info!("Cleared {} nodes from the graph", nodes);
            }

            CodeGraphAction::Query { cypher } => {
                if app.ctx.dry_run {
                    tracing::info!("DRY-RUN: Would execute query: {}", cypher);
                    return Ok(());
                }

                let result = client.query(cypher).await?;
                println!("{}", result);
            }

            CodeGraphAction::Stats => {
                let stats = client.get_stats().await?;
                println!("Graph Statistics:");
                println!("  Projects: {}", stats.total_projects);
                println!("  Files: {}", stats.total_files);
                println!("  Functions: {}", stats.total_functions);
                println!("  Structs: {}", stats.total_structs);
                println!("  Traits: {}", stats.total_traits);
            }
        }

        Ok(())
    }
}
