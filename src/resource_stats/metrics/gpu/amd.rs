//! AMD GPU metrics collector using rocm-smi

use async_trait::async_trait;
use tokio::process::Command;

use crate::resource_stats::metrics::{GpuCollector, MetricsError};
use crate::resource_stats::types::resource_stats::GpuResourceStats;

/// AMD GPU collector using rocm-smi
pub struct AmdGpuCollector {
    node_name: String,
}

impl AmdGpuCollector {
    pub fn new(node_name: String) -> Self {
        Self { node_name }
    }

    /// Check if rocm-smi is available
    fn check_rocm_smi() -> bool {
        std::process::Command::new("rocm-smi")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[async_trait]
impl GpuCollector for AmdGpuCollector {
    fn is_available(&self) -> bool {
        Self::check_rocm_smi()
    }

    fn vendor(&self) -> &'static str {
        "amd"
    }

    async fn collect(&self) -> Result<Vec<GpuResourceStats>, MetricsError> {
        // Query rocm-smi for GPU info
        let output = Command::new("rocm-smi")
            .args(["--showuse", "--showmeminfo", "vram", "--showtemp", "--showpower", "--json"])
            .output()
            .await
            .map_err(|e| MetricsError::Gpu(format!("Failed to run rocm-smi: {}", e)))?;

        if !output.status.success() {
            return Err(MetricsError::Gpu(format!(
                "rocm-smi failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse JSON output from rocm-smi
        let json: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| MetricsError::Parse(format!("Failed to parse rocm-smi JSON: {}", e)))?;

        let mut stats = Vec::new();

        // rocm-smi JSON format has GPU info under card keys like "card0", "card1", etc.
        if let Some(obj) = json.as_object() {
            for (key, value) in obj {
                if key.starts_with("card") {
                    let gpu_index = key
                        .trim_start_matches("card")
                        .parse::<i32>()
                        .unwrap_or(0);

                    let gpu_use = value
                        .get("GPU use (%)")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);

                    let vram_used = value
                        .get("VRAM Total Used Memory (B)")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or(0);

                    let vram_total = value
                        .get("VRAM Total Memory (B)")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or(0);

                    let temperature = value
                        .get("Temperature (Sensor edge) (C)")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .map(|t| t as i32);

                    let power = value
                        .get("Average Graphics Package Power (W)")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .map(|p| p as i32);

                    let model = value
                        .get("Card series")
                        .and_then(|v| v.as_str())
                        .unwrap_or("AMD GPU")
                        .to_string();

                    stats.push(GpuResourceStats {
                        node_name: self.node_name.clone(),
                        vendor: "amd".to_string(),
                        model,
                        gpu_index,
                        utilization_percent: gpu_use,
                        memory_used_bytes: vram_used,
                        memory_total_bytes: vram_total,
                        temperature_celsius: temperature,
                        power_watts: power,
                        cost_per_hour: None,
                    });
                }
            }
        }

        Ok(stats)
    }
}
