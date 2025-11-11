use clap::Subcommand;
use tokio::process::Command;
use crate::commands::RunCommand;
use crate::app::App;
use crate::utils::docker::resolve_compose_file;

#[derive(clap::Args)]
pub struct ComposeUpArgs {
    #[arg(short, long)]
    pub file: Option<String>,
    #[arg(short, long)]
    pub mam: Option<bool>,
    #[arg(short = 'd', long)]
    pub detach: bool,
    #[arg(last = true)]
    pub args: Vec<String>,
}

#[derive(clap::Args)]
pub struct ComposeDownArgs {
    #[arg(short, long)]
    pub file: Option<String>,
}

#[derive(clap::Args)]
pub struct ComposeConvertArgs {
    #[arg(short, long)]
    pub file: Option<String>,
}

#[derive(Subcommand)]
pub enum ComposeAction {
    Up(ComposeUpArgs),
    Down(ComposeDownArgs),
    Convert(ComposeConvertArgs),
}

#[async_trait::async_trait]
impl RunCommand for ComposeAction {
    async fn run(&self, app: &App) -> anyhow::Result<()> {
        match self {
            ComposeAction::Up(args) => {
                if app.ctx.dry_run {
                    tracing::info!("DRY-RUN: Would run docker compose up with detach={}", args.detach);
                    return Ok(());
                }

                if args.detach {
                    tracing::debug!("Running in detached mode");
                };
                let compose_file = resolve_compose_file(args.file.clone())?;

                let cmd = vec!["up"];
                let result = run_docker_compose(&cmd, &compose_file, args.detach, &args.args).await?;

                Ok(result)
            }
            ComposeAction::Down(args) => {
                if app.ctx.dry_run {
                    tracing::info!("DRY-RUN: Would run docker compose down");
                    return Ok(());
                }

                let compose_file = resolve_compose_file(args.file.clone())?;
                let result = run_docker_compose(&["down"], &compose_file, false, &[]).await;
                result
            }
            ComposeAction::Convert(args) => {
                if app.ctx.dry_run {
                    tracing::info!("DRY-RUN: Would convert docker compose to k8s");
                    return Ok(());
                }

                let compose_file = resolve_compose_file(args.file.clone())?;
                let result = run_kompose_convert(&["convert"], &compose_file).await;
                result
            }
        }
    }
}

async fn run_kompose_convert(args: &[&str], compose_file: &str) -> anyhow::Result<()> {
    let kompose_bin = std::env::var("DOTCONFIG_KOMPOSE_BIN").unwrap_or_else(|_| "kompose".into());
    let mut cmd = Command::new(kompose_bin);
    cmd.arg("--file").arg(compose_file);

    for arg in args {
        cmd.arg(arg);
    }

    let status = cmd.status().await?;

    if !status.success() {
        anyhow::bail!("Kompose convert command failed");
    }

    Ok(())
}

async fn run_docker_compose(
    args: &[&str],
    compose_file: &str,
    detach: bool,
    extra_args: &[String],
) -> anyhow::Result<()> {
    let docker_bin = std::env::var("DOTCONFIG_DOCKER_BIN").unwrap_or_else(|_| "docker".into());
    let mut cmd = Command::new(docker_bin);
    cmd.arg("compose").arg("-f").arg(compose_file);

    for arg in args {
        cmd.arg(arg);
    }

    if detach {
        cmd.arg("-d");
    }

    for arg in extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Docker compose command failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        println!("{}", stdout);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::RunCommand;
    use crate::context::{AppContext, OutputFormat};
    use crate::state::AppState;
    use crate::app::App;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_app() -> App {
        let ctx = AppContext::new(0, false, OutputFormat::Human, false, None, None, None);
        let state = AppState::new();
        App::new(ctx, state)
    }

    #[cfg(unix)]
    fn write_stub_script(
        dir: &std::path::Path,
        name: &str,
        script_content: &str,
    ) -> std::path::PathBuf {
        let script_path = dir.join(name);
        fs::write(&script_path, script_content).unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();
        script_path
    }

    #[tokio::test]
    #[serial]
    #[cfg(unix)]
    async fn test_handle_compose_up_happy_path() {
        let temp_dir = TempDir::new().unwrap();
        let compose_file = temp_dir.path().join("docker-compose.yml");
        fs::write(&compose_file, "version: '3'\n").unwrap();

        let stub_script = write_stub_script(
            temp_dir.path(),
            "docker",
            "#!/bin/bash\necho \"DOCKER_ARGS: $*\" > /dev/null\nexit 0\n",
        );

        std::env::set_var(
            "DOTCONFIG_DOCKER_BIN",
            stub_script.to_string_lossy().to_string(),
        );

        let app = create_test_app();
        let action = ComposeAction::Up(ComposeUpArgs {
            file: Some(compose_file.to_string_lossy().to_string()),
            detach: true,
            mam: Some(true),
            args: vec!["--build".to_string()],
        });
        let result = action.run(&app).await;

        std::env::remove_var("DOTCONFIG_DOCKER_BIN");

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[serial]
    #[cfg(unix)]
    async fn test_handle_compose_down_happy_path() {
        let temp_dir = TempDir::new().unwrap();
        let compose_file = temp_dir.path().join("docker-compose.yml");
        fs::write(&compose_file, "version: '3'\n").unwrap();

        let stub_script = write_stub_script(
            temp_dir.path(),
            "docker",
            "#!/bin/bash\necho \"DOCKER_ARGS: $*\" > /dev/null\nexit 0\n",
        );

        std::env::set_var(
            "DOTCONFIG_DOCKER_BIN",
            stub_script.to_string_lossy().to_string(),
        );

        let app = create_test_app();
        let action = ComposeAction::Down(ComposeDownArgs {
            file: Some(compose_file.to_string_lossy().to_string()),
        });
        let result = action.run(&app).await;

        std::env::remove_var("DOTCONFIG_DOCKER_BIN");

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[serial]
    #[cfg(unix)]
    async fn test_handle_compose_convert_happy_path() {
        let temp_dir = TempDir::new().unwrap();
        let compose_file = temp_dir.path().join("docker-compose.yml");
        fs::write(&compose_file, "version: '3'\n").unwrap();

        let stub_script = write_stub_script(
            temp_dir.path(),
            "kompose",
            "#!/bin/bash\necho \"KOMPOSE_ARGS: $*\" > /dev/null\nexit 0\n",
        );

        std::env::set_var(
            "DOTCONFIG_KOMPOSE_BIN",
            stub_script.to_string_lossy().to_string(),
        );

        let app = create_test_app();
        let action = ComposeAction::Convert(ComposeConvertArgs {
            file: Some(compose_file.to_string_lossy().to_string()),
        });
        let result = action.run(&app).await;

        std::env::remove_var("DOTCONFIG_KOMPOSE_BIN");

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_compose_file_not_found() {
        let app = create_test_app();
        let action = ComposeAction::Up(ComposeUpArgs {
            file: Some("nonexistent.yml".to_string()),
            detach: false,
            mam: None,
            args: vec![],
        });
        let result = action.run(&app).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Compose file not found: nonexistent.yml"));
    }

    #[tokio::test]
    #[serial]
    #[cfg(unix)]
    async fn test_handle_compose_docker_command_failure() {
        let temp_dir = TempDir::new().unwrap();
        let compose_file = temp_dir.path().join("docker-compose.yml");
        fs::write(&compose_file, "version: '3'\n").unwrap();

        let stub_script = write_stub_script(
            temp_dir.path(),
            "docker",
            "#!/bin/bash\necho \"boom\" >&2\nexit 3\n",
        );

        std::env::set_var(
            "DOTCONFIG_DOCKER_BIN",
            stub_script.to_string_lossy().to_string(),
        );

        let app = create_test_app();
        let action = ComposeAction::Up(ComposeUpArgs {
            file: Some(compose_file.to_string_lossy().to_string()),
            detach: false,
            mam: None,
            args: vec![],
        });
        let result = action.run(&app).await;

        std::env::remove_var("DOTCONFIG_DOCKER_BIN");

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Docker compose command failed"));
        assert!(error_msg.contains("boom"));
    }

    #[tokio::test]
    #[serial]
    #[cfg(unix)]
    async fn test_handle_compose_kompose_command_failure() {
        let temp_dir = TempDir::new().unwrap();
        let compose_file = temp_dir.path().join("docker-compose.yml");
        fs::write(&compose_file, "version: '3'\n").unwrap();

        let stub_script = write_stub_script(
            temp_dir.path(),
            "kompose",
            "#!/bin/bash\necho \"kompose error\" >&2\nexit 1\n",
        );

        std::env::set_var(
            "DOTCONFIG_KOMPOSE_BIN",
            stub_script.to_string_lossy().to_string(),
        );

        let app = create_test_app();
        let action = ComposeAction::Convert(ComposeConvertArgs {
            file: Some(compose_file.to_string_lossy().to_string()),
        });
        let result = action.run(&app).await;

        std::env::remove_var("DOTCONFIG_KOMPOSE_BIN");

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Kompose convert command failed"));
    }
}
