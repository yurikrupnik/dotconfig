mod path1;
mod crates;

use crates::{tracing::init_tracing};
mod commands;
// use commands::parse;
// pub mod crates;
// use crates::
// use clap::builder::styling;
use clap::{Parser, Subcommand};
// use std::path::Path;
// use std::process::Command;
use path1::{ShitAction, ComposeAction, DashboardAction, handle_compose, handle_shit, handle_dashboard};
mod errors;
// pub type Result<T> = core::result::Result<T, Errors>;

#[derive(Subcommand)]
pub enum Commands {
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
    // Yuri {
    //     #[command(subcommand)]
    //     action: DashboardAction,
    // }
    // Cluster {
    //     #[command(subcommand)]
    //     action: ClusterAction,
    // },
}

#[derive(Parser)]
#[command(name = "dotconfig")]
#[command(about = "Local development utilities")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
    #[arg(short='s', long)]
    dry_run: bool,
}

// use crates::cli::init_cli_app;

// fn init_app<T>() -> core::result::Result<T, Errors> {
//     let cli = Cli::parse();
//
//     match cli.command {
//         Commands::Compose { action } => handle_compose(action)?,
//         Commands::Shit { action } => handle_shit(action)?,
//         // Commands::ClusterAction { action } => handle_cluster(action)?,
//     }
// }




#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    // init_telemetry()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Compose { action } => handle_compose(action).await?,
        Commands::Shit { action } => handle_shit(action).await?,
        Commands::Dashboard { action } => handle_dashboard(action).await?,
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use clap::CommandFactory;
    // use Cli;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }
}
// struct Props {
//     name: String,
// }

// fn handle_cluster(action: ClusterAction) -> anyhow::Result<()> {
//     match action {
//         ClusterAction::Down { name } => {
//             let compose_file = resolve_compose_file(name)?;
//             run_docker_compose(&["up"], &compose_file)
//         }
//         // ClusterAction::Up { name } => {
//         //     let compose_file = resolve_compose_file(file)?;
//         //     run_docker_compose(&["up"], &compose_file)
//         // } // ClusterAction::Up { name, file }
//     }
// }




// fn run_docker_compose(args: &[&str], compose_file: &str) -> anyhow::Result<()> {
//     let mut cmd = Command::new("docker");
//     cmd.arg("compose").arg("-f").arg(compose_file);
//
//     for arg in args {
//         cmd.arg(arg);
//     }
//
//     let status = cmd.status()?;
//
//     if !status.success() {
//         anyhow::bail!("Docker compose command failed");
//     }
//
//     Ok(())
// }
