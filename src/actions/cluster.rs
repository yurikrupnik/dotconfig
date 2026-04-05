use crate::commands::RunCommand;
use crate::context::OutputFormat;
use crate::traits::CommandContext;
use clap::Subcommand;
use k8s_openapi::api::core::v1::Node;
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, Config};
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct ContextCheck {
    pub context: String,
    pub nodes: Vec<String>,
    pub healthy: bool,
    pub error: Option<String>,
}

#[derive(Subcommand)]
pub enum ClusterAction {
    /// Check all kubectl contexts and their node status
    Check {
        /// Timeout in seconds per context
        #[arg(short, long, default_value = "10")]
        timeout: u64,

        /// Only show healthy clusters
        #[arg(long)]
        healthy_only: bool,

        /// Only show unhClusterActionealthy clusters
        #[arg(long)]
        unhealthy_only: bool,
    },
    /// List all available contexts
    List,
    /// Show current context
    Current,
}

impl ClusterAction {
    async fn check_context(context_name: &str, timeout_secs: u64) -> ContextCheck {
        let kubeconfig = match Kubeconfig::read() {
            Ok(kc) => kc,
            Err(e) => {
                return ContextCheck {
                    context: context_name.to_string(),
                    nodes: vec![],
                    healthy: false,
                    error: Some(format!("Failed to read kubeconfig: {}", e)),
                };
            }
        };

        let options = KubeConfigOptions {
            context: Some(context_name.to_string()),
            ..Default::default()
        };

        let config = match Config::from_custom_kubeconfig(kubeconfig, &options).await {
            Ok(mut c) => {
                c.connect_timeout = Some(Duration::from_secs(timeout_secs));
                c.read_timeout = Some(Duration::from_secs(timeout_secs));
                c
            }
            Err(e) => {
                return ContextCheck {
                    context: context_name.to_string(),
                    nodes: vec![],
                    healthy: false,
                    error: Some(format!("Failed to create config: {}", e)),
                };
            }
        };

        let client = match Client::try_from(config) {
            Ok(c) => c,
            Err(e) => {
                return ContextCheck {
                    context: context_name.to_string(),
                    nodes: vec![],
                    healthy: false,
                    error: Some(format!("Failed to create client: {}", e)),
                };
            }
        };

        let api: Api<Node> = Api::all(client);

        match tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            api.list(&kube::api::ListParams::default()),
        )
        .await
        {
            Ok(Ok(nodes)) => {
                let node_names: Vec<String> = nodes
                    .items
                    .iter()
                    .filter_map(|n| n.metadata.name.clone())
                    .collect();

                if node_names.is_empty() {
                    ContextCheck {
                        context: context_name.to_string(),
                        nodes: vec![],
                        healthy: false,
                        error: Some("No nodes found".to_string()),
                    }
                } else {
                    ContextCheck {
                        context: context_name.to_string(),
                        nodes: node_names,
                        healthy: true,
                        error: None,
                    }
                }
            }
            Ok(Err(e)) => ContextCheck {
                context: context_name.to_string(),
                nodes: vec![],
                healthy: false,
                error: Some(format!("API error: {}", e)),
            },
            Err(_) => ContextCheck {
                context: context_name.to_string(),
                nodes: vec![],
                healthy: false,
                error: Some(format!("Timeout after {}s", timeout_secs)),
            },
        }
    }

    fn get_all_contexts() -> anyhow::Result<Vec<String>> {
        let kubeconfig = Kubeconfig::read()?;
        Ok(kubeconfig
            .contexts
            .into_iter()
            .map(|c| c.name)
            .collect())
    }

    fn get_current_context() -> anyhow::Result<String> {
        let kubeconfig = Kubeconfig::read()?;
        kubeconfig
            .current_context
            .ok_or_else(|| anyhow::anyhow!("No current context set"))
    }
}

#[async_trait::async_trait]
impl RunCommand for ClusterAction {
    async fn run(&self, ctx: &dyn CommandContext) -> anyhow::Result<()> {
        match self {
            ClusterAction::Check {
                timeout,
                healthy_only,
                unhealthy_only,
            } => {
                let contexts = Self::get_all_contexts()?;

                if contexts.is_empty() {
                    tracing::warn!("No kubectl contexts found");
                    return Ok(());
                }

                tracing::info!("Checking {} context(s)...", contexts.len());

                // Check all contexts concurrently
                let checks: Vec<_> = futures::future::join_all(
                    contexts
                        .iter()
                        .map(|c| Self::check_context(c, *timeout)),
                )
                .await;

                // Filter results
                let filtered: Vec<_> = checks
                    .into_iter()
                    .filter(|c| {
                        if *healthy_only {
                            c.healthy
                        } else if *unhealthy_only {
                            !c.healthy
                        } else {
                            true
                        }
                    })
                    .collect();

                match ctx.output_format() {
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&filtered)?);
                    }
                    OutputFormat::Human => {
                        println!();
                        println!("Cluster Health Check Results:");
                        println!("==============================");

                        for check in &filtered {
                            let status = if check.healthy { "✓" } else { "✗" };
                            let color = if check.healthy { "\x1b[32m" } else { "\x1b[31m" };
                            let reset = "\x1b[0m";

                            let info = if check.healthy {
                                format!("{} node(s)", check.nodes.len())
                            } else {
                                check.error.clone().unwrap_or_default()
                            };

                            println!("{}{}{} {}: {}", color, status, reset, check.context, info);
                        }

                        println!();

                        let healthy_count = filtered.iter().filter(|c| c.healthy).count();
                        let total = filtered.len();

                        if healthy_count == total {
                            tracing::info!("All {} clusters healthy", total);
                        } else {
                            tracing::warn!("{}/{} clusters healthy", healthy_count, total);
                        }
                    }
                    OutputFormat::Quiet => {}
                }

                Ok(())
            }
            ClusterAction::List => {
                let contexts = Self::get_all_contexts()?;
                let current = Self::get_current_context().ok();

                match ctx.output_format() {
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&contexts)?);
                    }
                    OutputFormat::Human => {
                        for c in contexts {
                            let marker = if Some(&c) == current.as_ref() {
                                "* "
                            } else {
                                "  "
                            };
                            println!("{}{}", marker, c);
                        }
                    }
                    OutputFormat::Quiet => {}
                }

                Ok(())
            }
            ClusterAction::Current => {
                let current = Self::get_current_context()?;

                match ctx.output_format() {
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string(&current)?);
                    }
                    OutputFormat::Human => {
                        println!("{}", current);
                    }
                    OutputFormat::Quiet => {}
                }

                Ok(())
            }
        }
    }
}
