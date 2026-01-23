//! Intel GPU metrics collector using intel_gpu_top or xpu-smi

use async_trait::async_trait;
use tokio::process::Command;

use crate::resource_stats::metrics::{GpuCollector, MetricsError};
use crate::resource_stats::types::resource_stats::GpuResourceStats;

/// Intel GPU collector using xpu-smi or intel_gpu_top
pub struct IntelGpuCollector {
    node_name: String,
}

impl IntelGpuCollector {
    pub fn new(node_name: String) -> Self {
        Self { node_name }
    }

    /// Check if xpu-smi is available (preferred for discrete GPUs)
    fn check_xpu_smi() -> bool {
        std::process::Command::new("xpu-smi")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if intel_gpu_top is available (for integrated GPUs)
    fn check_intel_gpu_top() -> bool {
        std::process::Command::new("intel_gpu_top")
            .arg("-h")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Collect using xpu-smi (for Intel Data Center GPUs)
    async fn collect_xpu_smi(&self) -> Result<Vec<GpuResourceStats>, MetricsError> {
        let output = Command::new("xpu-smi")
            .args(["discovery", "--json"])
            .output()
            .await
            .map_err(|e| MetricsError::Gpu(format!("Failed to run xpu-smi: {}", e)))?;

        if !output.status.success() {
            return Err(MetricsError::Gpu(format!(
                "xpu-smi failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let discovery: serde_json::Value =
            serde_json::from_slice(&output.stdout)
                .map_err(|e| MetricsError::Parse(format!("Failed to parse xpu-smi JSON: {}", e)))?;

        let mut stats = Vec::new();

        // Get device list
        if let Some(devices) = discovery.get("device_list").and_then(|d| d.as_array()) {
            for (idx, device) in devices.iter().enumerate() {
                let device_id = device
                    .get("device_id")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(idx as i64) as i32;

                let model = device
                    .get("device_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Intel GPU")
                    .to_string();

                // Get stats for this device
                let stats_output = Command::new("xpu-smi")
                    .args(["stats", "-d", &device_id.to_string(), "--json"])
                    .output()
                    .await
                    .ok();

                let (utilization, mem_used, mem_total, temp, power) =
                    if let Some(stats_out) = stats_output {
                        if let Ok(stats_json) = serde_json::from_slice::<serde_json::Value>(&stats_out.stdout) {
                            let util = stats_json
                                .get("gpu_util")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);
                            let mem_u = stats_json
                                .get("mem_used")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            let mem_t = stats_json
                                .get("mem_total")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            let t = stats_json
                                .get("temperature")
                                .and_then(|v| v.as_i64())
                                .map(|t| t as i32);
                            let p = stats_json
                                .get("power")
                                .and_then(|v| v.as_f64())
                                .map(|p| p as i32);
                            (util, mem_u, mem_t, t, p)
                        } else {
                            (0.0, 0, 0, None, None)
                        }
                    } else {
                        (0.0, 0, 0, None, None)
                    };

                stats.push(GpuResourceStats {
                    node_name: self.node_name.clone(),
                    vendor: "intel".to_string(),
                    model,
                    gpu_index: device_id,
                    utilization_percent: utilization,
                    memory_used_bytes: mem_used * 1024 * 1024, // Convert MiB to bytes
                    memory_total_bytes: mem_total * 1024 * 1024,
                    temperature_celsius: temp,
                    power_watts: power,
                    cost_per_hour: None,
                });
            }
        }

        Ok(stats)
    }
}

#[async_trait]
impl GpuCollector for IntelGpuCollector {
    fn is_available(&self) -> bool {
        Self::check_xpu_smi() || Self::check_intel_gpu_top()
    }

    fn vendor(&self) -> &'static str {
        "intel"
    }

    async fn collect(&self) -> Result<Vec<GpuResourceStats>, MetricsError> {
        // Prefer xpu-smi for discrete GPUs
        if Self::check_xpu_smi() {
            return self.collect_xpu_smi().await;
        }

        // Fallback to intel_gpu_top for integrated GPUs
        // Note: intel_gpu_top requires special permissions and is more complex to parse
        // For now, return empty if xpu-smi is not available
        Err(MetricsError::Unavailable(
            "Intel GPU tools not available. Install xpu-smi for Intel Data Center GPUs".into(),
        ))
    }
}
