use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;
use pg_gen::ConfigMapBuilder;

#[derive(Parser)]
#[command(name = "pg-cli", about = "Playground CLI — config & manifest generation")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate Kubernetes ConfigMaps from SQL files
    #[command(subcommand)]
    Configmap(ConfigmapCommands),
}

#[derive(Subcommand)]
enum ConfigmapCommands {
    /// Generate schema ConfigMap from schema.sql
    Schema {
        /// Path to schema.sql
        #[arg(long, default_value = "manifests/db/schema.sql")]
        schema: PathBuf,

        /// ConfigMap name
        #[arg(long, default_value = "mydatabase-schema")]
        name: String,

        /// ConfigMap namespace
        #[arg(long, default_value = "dbs")]
        namespace: String,

        /// Write output to file(s) instead of stdout
        #[arg(short, long)]
        output: Vec<PathBuf>,
    },

    /// Generate migrations ConfigMap from migration SQL files
    Migrations {
        /// Path to migrations directory
        #[arg(long, default_value = "manifests/db/migrations")]
        migrations_dir: PathBuf,

        /// ConfigMap name
        #[arg(long, default_value = "mydatabase-migrations")]
        name: String,

        /// Write output to file(s) instead of stdout
        #[arg(short, long)]
        output: Vec<PathBuf>,
    },
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    match cli.command {
        Commands::Configmap(cmd) => match cmd {
            ConfigmapCommands::Schema {
                schema,
                name,
                namespace,
                output,
            } => {
                let cm = ConfigMapBuilder::new(&name)
                    .namespace(&namespace)
                    .label("app.kubernetes.io/name", &name)
                    .label("app.kubernetes.io/component", "schema")
                    .data_from_file("schema.sql", &schema)?
                    .build();
                let yaml = cm.to_yaml()?;
                write_or_print(&yaml, &output)?;
            }
            ConfigmapCommands::Migrations {
                migrations_dir,
                name,
                output,
            } => {
                let cm = ConfigMapBuilder::new(&name)
                    .label("app.kubernetes.io/name", &name)
                    .label("app.kubernetes.io/component", "migration")
                    .data_from_migration_dir(&migrations_dir)?
                    .build();
                let yaml = cm.to_yaml()?;
                write_or_print(&yaml, &output)?;
            }
        },
    }

    Ok(())
}

fn write_or_print(yaml: &str, outputs: &[PathBuf]) -> Result<()> {
    if outputs.is_empty() {
        print!("{yaml}");
    } else {
        for path in outputs {
            std::fs::write(path, yaml)?;
            eprintln!("Wrote {}", path.display());
        }
    }
    Ok(())
}
