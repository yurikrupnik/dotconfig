use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Local development utilities"));
}

#[test]
fn test_cli_version() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("--version");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("dotconfig"));
}

#[test]
fn test_cli_invalid_command() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("invalid-command");

    cmd.assert().failure();
}

#[test]
fn test_compose_help() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("compose").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("compose"));
}

#[test]
fn test_dry_run_flag() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    let temp_dir = TempDir::new().unwrap();
    let compose_file = temp_dir.path().join("docker-compose.yml");
    fs::write(&compose_file, "version: '3'\nservices: {}").unwrap();

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
fn test_debug_flag() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("-d").arg("--help");

    cmd.assert().success();
}

#[test]
fn test_multiple_debug_flags() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("-dd").arg("--help");

    cmd.assert().success();
}

#[test]
fn test_output_format_json() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("--output").arg("json").arg("--help");

    cmd.assert().success();
}

#[test]
fn test_output_format_quiet() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("--output").arg("quiet").arg("--help");

    cmd.assert().success();
}

#[test]
fn test_no_color_flag() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("--no-color").arg("--help");

    cmd.assert().success();
}

#[test]
fn test_compose_up_without_file() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("compose").arg("up");

    cmd.assert().failure();
}

#[test]
fn test_compose_down_help() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("compose").arg("down").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("down"));
}

#[test]
fn test_compose_convert_help() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("compose").arg("convert").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("convert"));
}

#[test]
fn test_code_graph_help() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("code-graph").arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("code-graph"));
}

#[test]
fn test_completions_bash() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("completions").arg("bash");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("complete"));
}

#[test]
fn test_completions_zsh() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("completions").arg("zsh");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("compdef"));
}

#[test]
fn test_completions_fish() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("completions").arg("fish");

    cmd.assert().success();
}

#[test]
fn test_env_var_postgres_url() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.env("POSTGRES_URL", "postgres://test:5432/db")
        .arg("--help");

    cmd.assert().success();
}

#[test]
fn test_env_var_redis_url() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.env("REDIS_URL", "redis://test:6379").arg("--help");

    cmd.assert().success();
}

#[test]
fn test_cli_arg_postgres_url() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("--postgres-url")
        .arg("postgres://cli:5432/db")
        .arg("--help");

    cmd.assert().success();
}

#[test]
fn test_config_file_not_found() {
    let mut cmd = Command::cargo_bin("dotconfig").unwrap();
    cmd.arg("--config")
        .arg("nonexistent_config.toml")
        .arg("--help");

    cmd.assert().success();
}
