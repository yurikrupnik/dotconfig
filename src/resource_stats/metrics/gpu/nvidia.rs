//! NVIDIA GPU metrics collector

use async_trait::async_trait;
use serde::Deserialize;
use tokio::process::Command;

use crate::resource_stats::metrics::{GpuCollector, MetricsError};
use crate::resource_stats::types::resource_stats::GpuResourceStats;

/// NVIDIA GPU collector using nvidia-smi
pub struct NvidiaGpuCollector {
    node_name: String,
}

impl NvidiaGpuCollector {
    pub fn new(node_name: String) -> Self {
        Self { node_name }
    }

    /// Check if nvidia-smi is available
    fn check_nvidia_smi() -> bool {
        std::process::Command::new("nvidia-smi")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[async_trait]
impl GpuCollector for NvidiaGpuCollector {
    fn is_available(&self) -> bool {
        Self::check_nvidia_smi()
    }

    fn vendor(&self) -> &'static str {
        "nvidia"
    }

    async fn collect(&self) -> Result<Vec<GpuResourceStats>, MetricsError> {
        // Query nvidia-smi in CSV format
        let output = Command::new("nvidia-smi")
            .args([
                "--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .await
            .map_err(|e| MetricsError::Gpu(format!("Failed to run nvidia-smi: {}", e)))?;

        if !output.status.success() {
            return Err(MetricsError::Gpu(format!(
                "nvidia-smi failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut stats = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 5 {
                let gpu_index = parts[0].parse::<i32>().unwrap_or(0);
                let model = parts[1].to_string();
                let utilization = parts[2].parse::<f64>().unwrap_or(0.0);
                let memory_used_mib = parts[3].parse::<i64>().unwrap_or(0);
                let memory_total_mib = parts[4].parse::<i64>().unwrap_or(0);
                let temperature = parts.get(5).and_then(|t| t.parse::<i32>().ok());
                let power = parts.get(6).and_then(|p| p.parse::<f64>().ok()).map(|p| p as i32);

                stats.push(GpuResourceStats {
                    node_name: self.node_name.clone(),
                    vendor: "nvidia".to_string(),
                    model,
                    gpu_index,
                    utilization_percent: utilization,
                    memory_used_bytes: memory_used_mib * 1024 * 1024,
                    memory_total_bytes: memory_total_mib * 1024 * 1024,
                    temperature_celsius: temperature,
                    power_watts: power,
                    cost_per_hour: None,
                });
            }
        }

        Ok(stats)
    }
}

/// NVIDIA GPU info from nvidia-smi query
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct NvidiaGpuInfo {
    index: i32,
    name: String,
    utilization_gpu: f64,
    memory_used: i64,
    memory_total: i64,
    temperature_gpu: Option<i32>,
    power_draw: Option<f64>,
}
