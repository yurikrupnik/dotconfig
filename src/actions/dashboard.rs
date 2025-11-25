use crate::commands::RunCommand;
use crate::traits::CommandContext;
use clap::Subcommand;
use std::fs;

#[derive(Subcommand)]
pub enum DashboardAction {
    Create {
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        query: String,
    },
    List,
}

#[async_trait::async_trait]
impl RunCommand for DashboardAction {
    async fn run(&self, ctx: &dyn CommandContext) -> anyhow::Result<()> {
        match self {
            DashboardAction::Create { name, query } => {
                let dashboard_template = serde_json::json!({
                    "dashboard": {
                        "id": null,
                        "title": name,
                        "tags": ["custom", "cli-generated"],
                        "style": "dark",
                        "timezone": "browser",
                        "panels": [
                            {
                                "id": 1,
                                "title": format!("{} Panel", name),
                                "type": "timeseries",
                                "targets": [
                                    {
                                        "datasource": {
                                            "type": "influxdb",
                                            "uid": "influxdb"
                                        },
                                        "query": query,
                                        "refId": "A"
                                    }
                                ],
                                "fieldConfig": {
                                    "defaults": {
                                        "color": {
                                            "mode": "palette-classic"
                                        }
                                    }
                                },
                                "gridPos": {"h": 8, "w": 24, "x": 0, "y": 0}
                            }
                        ],
                        "time": {
                            "from": "now-1h",
                            "to": "now"
                        },
                        "refresh": "5s",
                        "schemaVersion": 27,
                        "version": 0
                    }
                });

                let dashboard_path = format!(
                    "./scripts/nu/local-dev/grafana/provisioning/dashboards/{}.json",
                    name
                );
                let dashboard_content = serde_json::to_string_pretty(&dashboard_template)?;

                if ctx.dry_run() {
                    tracing::info!(
                        "DRY-RUN: Would create dashboard '{}' at {}",
                        name,
                        dashboard_path
                    );
                    return Ok(());
                }

                let result = fs::write(&dashboard_path, dashboard_content);

                match result {
                    Ok(_) => {
                        tracing::info!(
                            "Dashboard '{}' created successfully at {}",
                            name,
                            dashboard_path
                        );
                        Ok(())
                    }
                    Err(e) => anyhow::bail!("Failed to create dashboard: {}", e),
                }
            }
            DashboardAction::List => {
                let dashboard_dir = "./scripts/nu/local-dev/grafana/provisioning/dashboards";

                let result = fs::read_dir(dashboard_dir);

                match result {
                    Ok(entries) => {
                        println!("Available dashboards:");
                        for entry in entries.flatten() {
                            if let Some(filename) = entry.file_name().to_str() {
                                if filename.ends_with(".json") {
                                    println!("  - {}", filename.trim_end_matches(".json"));
                                }
                            }
                        }
                        Ok(())
                    }
                    Err(e) => anyhow::bail!("Failed to list dashboards: {}", e),
                }
            }
        }
    }
}
