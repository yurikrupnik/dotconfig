use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::io::Write;
use tempfile::{NamedTempFile, TempDir};

#[test]
#[cfg(unix)]
fn test_compose_up_e2e_with_stub_docker() {
    let temp_dir = TempDir::new().unwrap();
    let compose_file = temp_dir.path().join("docker-compose.yml");
    fs::write(
        &compose_file,
        "version: '3'\nservices:\n  test:\n    image: alpine\n",
    )
    .unwrap();

    let stub_docker = temp_dir.path().join("docker");
    fs::write(
        &stub_docker,
        "#!/bin/bash\necho \"Docker stub called with: $@\"\nexit 0\n",
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&stub_docker, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.env("DOTCONFIG_DOCKER_BIN", stub_docker.to_str().unwrap())
        .arg("compose")
        .arg("up")
        .arg("--file")
        .arg(compose_file.to_str().unwrap())
        .arg("--detach");

    cmd.assert().success();
}

#[test]
#[cfg(unix)]
fn test_compose_down_e2e_with_stub_docker() {
    let temp_dir = TempDir::new().unwrap();
    let compose_file = temp_dir.path().join("docker-compose.yml");
    fs::write(&compose_file, "version: '3'\nservices: {}\n").unwrap();

    let stub_docker = temp_dir.path().join("docker");
    fs::write(
        &stub_docker,
        "#!/bin/bash\necho \"Docker down\"\nexit 0\n",
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&stub_docker, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.env("DOTCONFIG_DOCKER_BIN", stub_docker.to_str().unwrap())
        .arg("compose")
        .arg("down")
        .arg("--file")
        .arg(compose_file.to_str().unwrap());

    cmd.assert().success();
}

#[test]
#[cfg(unix)]
fn test_compose_convert_e2e_with_stub_kompose() {
    let temp_dir = TempDir::new().unwrap();
    let compose_file = temp_dir.path().join("docker-compose.yml");
    fs::write(&compose_file, "version: '3'\nservices: {}\n").unwrap();

    let stub_kompose = temp_dir.path().join("kompose");
    fs::write(
        &stub_kompose,
        "#!/bin/bash\necho \"Kompose convert\"\nexit 0\n",
    )
    .unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&stub_kompose, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.env("DOTCONFIG_KOMPOSE_BIN", stub_kompose.to_str().unwrap())
        .arg("compose")
        .arg("convert")
        .arg("--file")
        .arg(compose_file.to_str().unwrap());

    cmd.assert().success();
}

#[test]
fn test_config_file_loading_e2e() {
    let mut temp_config = NamedTempFile::new().unwrap();
    let config_content = r#"
[database]
postgres_url = "postgres://localhost:5432/testdb"
redis_url = "redis://localhost:6379"

[logging]
level = "debug"
format = "json"
"#;
    temp_config.write_all(config_content.as_bytes()).unwrap();
    temp_config.flush().unwrap();

    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("--config")
        .arg(temp_config.path().to_str().unwrap())
        .arg("--help");

    cmd.assert().success();
}

#[test]
fn test_compose_file_precedence_e2e() {
    let temp_dir = TempDir::new().unwrap();

    fs::write(
        temp_dir.path().join("docker-compose.yml"),
        "version: '3'\nservices: {}\n",
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.current_dir(&temp_dir)
        .arg("--dry-run")
        .arg("compose")
        .arg("up");

    cmd.assert().success();
}

#[test]
fn test_debug_levels_e2e() {
    let mut cmd1 = Command::cargo_bin("dotconfig").unwrap();
    cmd1.arg("-d").arg("--help");
    cmd1.assert().success();

    let mut cmd2 = Command::cargo_bin("dotconfig").unwrap();
    cmd2.arg("-dd").arg("--help");
    cmd2.assert().success();

    let mut cmd3 = Command::cargo_bin("dotconfig").unwrap();
    cmd3.arg("-ddd").arg("--help");
    cmd3.assert().success();
}

#[test]
fn test_env_overrides_e2e() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.env("POSTGRES_URL", "postgres://env:5432/db")
        .arg("--postgres-url")
        .arg("postgres://cli:5432/db")
        .arg("--help");

    cmd.assert().success();
}

#[test]
fn test_compose_missing_file_e2e() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("compose")
        .arg("up")
        .arg("--file")
        .arg("nonexistent-compose.yml");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Compose file not found"));
}

#[test]
fn test_dry_run_prevents_execution() {
    let temp_dir = TempDir::new().unwrap();
    let compose_file = temp_dir.path().join("docker-compose.yml");
    fs::write(&compose_file, "version: '3'\nservices: {}\n").unwrap();

    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("--dry-run")
        .arg("compose")
        .arg("up")
        .arg("--file")
        .arg(compose_file.to_str().unwrap());

    cmd.assert().success().stdout(
        predicate::str::contains("DRY-RUN").or(predicate::str::contains("Running in DRY-RUN mode")),
    );
}

#[test]
fn test_multiple_commands_help() {
    let commands = vec!["compose", "shit", "dashboard", "code-graph"];

    for command in commands {
        let mut cmd = Command::cargo_bin("dotconfig").unwrap();
        cmd.arg(command).arg("--help");

        cmd.assert().success();
    }
}

#[test]
fn test_completions_all_shells() {
    let shells = vec!["bash", "zsh", "fish", "powershell", "elvish"];

    for shell in shells {
        let mut cmd = Command::cargo_bin("dotconfig").unwrap();
        cmd.arg("completions").arg(shell);

        cmd.assert().success();
    }
}

#[test]
fn test_output_formats_e2e() {
    let formats = vec!["human", "json", "quiet"];

    for format in formats {
        let mut cmd = Command::cargo_bin("dotconfig").unwrap();
        cmd.arg("--output").arg(format).arg("--help");

        cmd.assert().success();
    }
}
