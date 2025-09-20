// use std::path::Path;
// use std::process::Command;
// use clap::Subcommand;
//
// #[derive(Subcommand)]
// pub enum Commands {
//     Compose {
//         #[command(subcommand)]
//         action: ComposeAction,
//     },
//     // Shit {
//     //     #[command(subcommand)]
//     //     action: ShitAction,
//     // }
//     // Cluster {
//     //     #[command(subcommand)]
//     //     action: ClusterAction,
//     // },
// }
//
// #[derive(Parser)]
// #[command(name = "dotconfig")]
// #[command(about = "Local development utilities")]
// pub struct Cli {
//     #[command(subcommand)]
//     command: Commands,
// }

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

pub fn manage() -> Result<()>{
    let cli = Cli::parse();

    match cli.command {
        Commands::Compose { action } => handle_compose(action)?,
        Commands::Shit { action } => handle_shit(action)?,
        // Commands::ClusterAction { action } => handle_cluster(action)?,
    }
}

#[derive(Subcommand)]
pub enum Commands {
    Compose {
        #[command(subcommand)]
        action: ComposeAction,
    },
    Shit {
        #[command(subcommand)]
        action: ShitAction,
    }
    // Cluster {
    //     #[command(subcommand)]
    //     action: ClusterAction,
    // },
}