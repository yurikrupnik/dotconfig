use crate::operator::types::{HelmSpec, HelmReleaseStatus};
use crate::operator::{OperatorError, Result};
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Helm client for managing Helm releases
pub struct HelmClient {
    /// Path to Helm binary
    helm_binary: String,

    /// Optional kubeconfig path
    kubeconfig: Option<String>,
}

impl HelmClient {
    /// Create a new Helm client with default configuration
    pub fn new() -> Self {
        Self {
            helm_binary: std::env::var("HELM_BINARY").unwrap_or_else(|_| "helm".to_string()),
            kubeconfig: std::env::var("KUBECONFIG").ok(),
        }
    }

    /// Create a new Helm client with custom binary path
    pub fn with_binary(binary: impl Into<String>) -> Self {
        Self {
            helm_binary: binary.into(),
            kubeconfig: std::env::var("KUBECONFIG").ok(),
        }
    }

    /// Install or upgrade a Helm release
    pub async fn install_or_upgrade(
        &self,
        name: &str,
        namespace: &str,
        spec: &HelmSpec,
    ) -> Result<HelmReleaseStatus> {
        let mut args = vec![
            "upgrade".to_string(),
            "--install".to_string(),
            name.to_string(),
            spec.chart.clone(),
            "--namespace".to_string(),
            namespace.to_string(),
            "--create-namespace".to_string(),
            "--output".to_string(),
            "json".to_string(),
        ];

        // Add wait flag
        if spec.wait {
            args.push("--wait".to_string());
        }

        // Add timeout
        args.push("--timeout".to_string());
        args.push(format!("{}s", spec.timeout));

        // Add repository if specified
        if let Some(repo) = &spec.repository {
            args.push("--repo".to_string());
            args.push(repo.clone());
        }

        // Add version if specified
        if let Some(version) = &spec.version {
            args.push("--version".to_string());
            args.push(version.clone());
        }

        // Add values as JSON - keep temp_file alive until after command execution
        let _temp_file = if let Some(values) = &spec.values {
            let values_json = serde_json::to_string(values)
                .map_err(|e| OperatorError::Helm(format!("Failed to serialize values: {}", e)))?;

            // Write values to temp file for complex values
            let temp_file = tempfile::NamedTempFile::new()
                .map_err(|e| OperatorError::Helm(format!("Failed to create temp file: {}", e)))?;

            std::fs::write(temp_file.path(), &values_json)
                .map_err(|e| OperatorError::Helm(format!("Failed to write values file: {}", e)))?;

            args.push("--values".to_string());
            args.push(temp_file.path().to_string_lossy().to_string());

            Some(temp_file)
        } else {
            None
        };

        debug!("Executing Helm: {} {}", self.helm_binary, args.join(" "));

        let mut cmd = Command::new(&self.helm_binary);
        cmd.args(&args);

        if let Some(kubeconfig) = &self.kubeconfig {
            cmd.env("KUBECONFIG", kubeconfig);
        }

        let output = cmd.output().await
            .map_err(|e| OperatorError::Helm(format!("Failed to execute Helm: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OperatorError::Helm(format!("Helm upgrade failed: {}", stderr)));
        }

        // Parse JSON output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let release_info: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| OperatorError::Helm(format!("Failed to parse Helm output: {}", e)))?;

        let revision = release_info.get("version")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as i32;

        let status_str = release_info.get("info")
            .and_then(|i| i.get("status"))
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string();

        let chart_version = release_info.get("chart")
            .and_then(|c| c.get("metadata"))
            .and_then(|m| m.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        info!("Helm release {} installed/upgraded to revision {}", name, revision);

        Ok(HelmReleaseStatus {
            revision,
            status: status_str,
            last_applied: Some(chrono::Utc::now().to_rfc3339()),
            chart_version,
        })
    }

    /// Get the status of a Helm release
    pub async fn get_status(
        &self,
        name: &str,
        namespace: &str,
    ) -> Result<Option<HelmReleaseStatus>> {
        let args = vec![
            "status".to_string(),
            name.to_string(),
            "--namespace".to_string(),
            namespace.to_string(),
            "--output".to_string(),
            "json".to_string(),
        ];

        let mut cmd = Command::new(&self.helm_binary);
        cmd.args(&args);

        if let Some(kubeconfig) = &self.kubeconfig {
            cmd.env("KUBECONFIG", kubeconfig);
        }

        let output = cmd.output().await
            .map_err(|e| OperatorError::Helm(format!("Failed to execute Helm status: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Release doesn't exist
            if stderr.contains("not found") {
                return Ok(None);
            }
            return Err(OperatorError::Helm(format!("Helm status failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let release_info: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| OperatorError::Helm(format!("Failed to parse Helm status: {}", e)))?;

        let revision = release_info.get("version")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as i32;

        let status_str = release_info.get("info")
            .and_then(|i| i.get("status"))
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Some(HelmReleaseStatus {
            revision,
            status: status_str,
            last_applied: None,
            chart_version: None,
        }))
    }

    /// Uninstall a Helm release
    pub async fn uninstall(
        &self,
        name: &str,
        namespace: &str,
    ) -> Result<()> {
        let args = vec![
            "uninstall".to_string(),
            name.to_string(),
            "--namespace".to_string(),
            namespace.to_string(),
            "--wait".to_string(),
        ];

        let mut cmd = Command::new(&self.helm_binary);
        cmd.args(&args);

        if let Some(kubeconfig) = &self.kubeconfig {
            cmd.env("KUBECONFIG", kubeconfig);
        }

        let output = cmd.output().await
            .map_err(|e| OperatorError::Helm(format!("Failed to execute Helm uninstall: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ignore if release doesn't exist
            if stderr.contains("not found") {
                warn!("Helm release {} not found, skipping uninstall", name);
                return Ok(());
            }
            return Err(OperatorError::Helm(format!("Helm uninstall failed: {}", stderr)));
        }

        info!("Helm release {} uninstalled", name);

        Ok(())
    }

    /// List all Helm releases in a namespace
    pub async fn list(
        &self,
        namespace: Option<&str>,
    ) -> Result<Vec<HelmReleaseStatus>> {
        let mut args = vec![
            "list".to_string(),
            "--output".to_string(),
            "json".to_string(),
        ];

        if let Some(ns) = namespace {
            args.push("--namespace".to_string());
            args.push(ns.to_string());
        } else {
            args.push("--all-namespaces".to_string());
        }

        let mut cmd = Command::new(&self.helm_binary);
        cmd.args(&args);

        if let Some(kubeconfig) = &self.kubeconfig {
            cmd.env("KUBECONFIG", kubeconfig);
        }

        let output = cmd.output().await
            .map_err(|e| OperatorError::Helm(format!("Failed to execute Helm list: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OperatorError::Helm(format!("Helm list failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let releases: Vec<serde_json::Value> = serde_json::from_str(&stdout)
            .map_err(|e| OperatorError::Helm(format!("Failed to parse Helm list: {}", e)))?;

        let statuses: Vec<HelmReleaseStatus> = releases.iter().map(|r| {
            HelmReleaseStatus {
                revision: r.get("revision")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1) as i32,
                status: r.get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                last_applied: r.get("updated")
                    .and_then(|u| u.as_str())
                    .map(|s| s.to_string()),
                chart_version: r.get("chart")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string()),
            }
        }).collect();

        Ok(statuses)
    }
}

impl Default for HelmClient {
    fn default() -> Self {
        Self::new()
    }
}
