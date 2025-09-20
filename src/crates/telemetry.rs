// use influxdb2::models::DataPoint;
// use influxdb2::Client;
// use serde::{Deserialize, Serialize};
// use chrono::{DateTime, Utc};
// use std::collections::HashMap;
//
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct TelemetryEvent {
//     pub command: String,
//     pub args: Vec<String>,
//     pub duration_ms: u64,
//     pub success: bool,
//     pub error_message: Option<String>,
//     pub timestamp: DateTime<Utc>,
//     pub tags: HashMap<String, String>,
// }
//
// #[derive(Debug, Clone)]
// pub struct TelemetryConfig {
//     pub url: String,
//     pub token: String,
//     pub org: String,
//     pub bucket: String,
// }
//
// impl Default for TelemetryConfig {
//     fn default() -> Self {
//         Self {
//             url: "http://localhost:8086".to_string(),
//             token: "my-super-secret-auth-token".to_string(),
//             org: "dotconfig".to_string(),
//             bucket: "telemetry".to_string(),
//         }
//     }
// }
//
// pub struct TelemetryCollector {
//     client: Client,
//     config: TelemetryConfig,
// }
//
// impl TelemetryCollector {
//     pub fn new(config: TelemetryConfig) -> Self {
//         let client = Client::new(&config.url, &config.org, &config.token);
//         Self { client, config }
//     }
//
//     pub async fn record_event(&self, event: TelemetryEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//         let mut point = DataPoint::builder("cli_command")
//             .tag("command", &event.command)
//             .tag("success", &event.success.to_string())
//             .field("duration_ms", event.duration_ms as i64)
//             .timestamp(event.timestamp.timestamp_nanos_opt().unwrap_or(0));
//
//         for arg in &event.args {
//             point = point.tag("arg", arg);
//         }
//
//         for (key, value) in &event.tags {
//             point = point.tag(key, value);
//         }
//
//         if let Some(error) = &event.error_message {
//             point = point.field("error_message", error.clone());
//         }
//
//         let point = point.build()?;
//
//         self.client
//             .write(&self.config.bucket, futures::stream::iter([point]))
//             .await
//             .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
//     }
//
//     pub async fn record_command_start(&self, command: &str, args: &[String]) -> TelemetryEvent {
//         TelemetryEvent {
//             command: command.to_string(),
//             args: args.to_vec(),
//             duration_ms: 0,
//             success: false,
//             error_message: None,
//             timestamp: Utc::now(),
//             tags: HashMap::new(),
//         }
//     }
//
//     pub async fn record_command_end(&self, mut event: TelemetryEvent, success: bool, error: Option<String>, duration_ms: u64) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//         event.success = success;
//         event.error_message = error;
//         event.duration_ms = duration_ms;
//
//         self.record_event(event).await
//     }
// }
//
// pub static mut TELEMETRY_COLLECTOR: Option<TelemetryCollector> = None;
//
// pub fn init_telemetry() -> anyhow::Result<()> {
//     let config = TelemetryConfig::default();
//     let collector = TelemetryCollector::new(config);
//
//     unsafe {
//         TELEMETRY_COLLECTOR = Some(collector);
//     }
//
//     Ok(())
// }
//
// pub fn get_telemetry_collector() -> Option<&'static TelemetryCollector> {
//     unsafe { TELEMETRY_COLLECTOR.as_ref() }
// }