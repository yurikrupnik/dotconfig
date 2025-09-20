// use std::env;
// use tracing::info;
// use tracing_subscriber::EnvFilter;
// use clap::{Parser, Subcommand};
// use crate::{Cli, Commands};
// use crate::path1::{handle_compose, handle_shit};
// pub async fn init_cli_app() {
//     let cli = Cli::parse();
// 
//     match cli.command {
//         Commands::Compose { action } => handle_compose(action)?,
//         Commands::Shit { action } => handle_shit(action)?,
//         // Commands::ClusterAction { action } => handle_cluster(action)?,
//     }
// }