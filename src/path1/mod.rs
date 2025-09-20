use std::path::Path;
use std::process::Command;
use clap::Subcommand;
// use k8s_openapi::api::core::v1::{Container, Namespace, Pod, PodSpec, Probe};
// use kube::{
//     api::{Api, ListParams, PostParams},
//     Client,
// };
use serde::{Deserialize};
// use crate::telemetry::{get_telemetry_collector, TelemetryEvent};
use chrono::Utc;
use std::time::Instant;
use std::fs;
// use crate::{resolve_compose_file, run_docker_compose};
// use commands::Cli;
// mod commands;
#[derive(Deserialize)]
struct PodSpecInput {
    name: String,
    image: String,
}

#[derive(Deserialize)]
struct NamespacePath {
    namespace: String,
}

// async fn create_pod_handler(namespace: Path<String>, pod_spec: Json<PodSpecInput>) -> HttpResponse {
//     let client = Client::try_default()
//         .await
//         .expect("Failed to create client");
//     // let namespace = req.match_info().get("namespace").unwrap();
//     // let namespace = "aris";
//     // println!("namespace: {}", namespace);
//     // HttpResponse::Ok().finish()
//     let ns = namespace.into_inner();
//     create_pod(client, ns, pod_spec.into_inner()).await
// }

fn resolve_compose_file(file: Option<String>) -> anyhow::Result<String> {
    if let Some(file_path) = file {
        if Path::new(&file_path).exists() {
            Ok(file_path)
        } else {
            anyhow::bail!("Compose file not found: {}", file_path);
        }
    } else {
        let compose_files = [
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml",
            "compose.yaml",
        ];

        for file in &compose_files {
            if Path::new(file).exists() {
                return Ok(file.to_string());
            }
        }

        Ok("./compose.yaml".to_string())
    }
}

// #[derive(Subcommand)]
// pub enum Commands {
//     Compose {
//         #[command(subcommand)]
//         action: ComposeAction,
//     },
//     Shit {
//         #[command(subcommand)]
//         action: ShitAction,
//     }
//     // Cluster {
//     //     #[command(subcommand)]
//     //     action: ClusterAction,
//     // },
// }

// #[derive(Parser, Debug)]
// pub struct DoItProps {
    // #[clap(about = "System name")]
    // #[arg(short, long)]
    // name: String,
// }

// async fn create_pod(client: Client, namespace: String, pod_spec: PodSpecInput) -> HttpResponse {
//     let pods: Api<Pod> = Api::namespaced(client, &namespace);
//
//     let pod = Pod {
//         metadata: kube::api::ObjectMeta {
//             name: Some(pod_spec.name.clone()),
//             ..Default::default()
//         },
//         spec: Some(PodSpec {
//             containers: vec![Container {
//                 name: pod_spec.name.clone(),
//                 image: Some(pod_spec.image.clone()),
//                 ..Default::default() // liveness_probe: Probe {
//                 //     http_get: {}, // exec:
//                 // }, // liveness_probe: {}..Default::default(),
//             }],
//             ..Default::default()
//         }),
//         ..Default::default()
//     };
//
//     // match pods.delete(&PostParams::default(), &pod) {  }
//     match pods.create(&PostParams::default(), &pod).await {
//         Ok(o) => HttpResponse::Ok().json(&o),
//         Err(e) => HttpResponse::InternalServerError().body(format!("Error creating pod: {:?}", e)),
//     }
// }


pub async fn handle_shit(action: ShitAction) -> anyhow::Result<()> {
    match action {
        ShitAction::DoIt { name } => {
            println!("name is {name:?}");
            // let compose_file = resolve_compose_file(file)?;
            // run_docker_compose(&["up"], &compose_file)
            let mut cmd = Command::new("ls");
            // cmd.arg("compose").arg("-f").arg(compose_file);

            // for arg in args {
            //     cmd.arg(arg);
            // }

            let status = cmd.status()?;

            if !status.success() {
                anyhow::bail!("Docker compose command failed");
            }

            Ok(())
        },
        ShitAction::Project { name, dry_run, docker_runtime } => {
            println!("name is {name:?}");
            println!("dry_run is {dry_run:?}");
            println!("docker_runtime is {docker_runtime:?}");
            let mut cmd = Command::new("ls");
            // cmd.arg("compose").arg("-f").arg(compose_file);

            // for arg in args {
            //     cmd.arg(arg);
            // }

            let status = cmd.status()?;

            if !status.success() {
                anyhow::bail!("Docker compose command failed");
            }

            Ok(())
        },
        // ComposeAction::Down { file } => {
        //     let compose_file = resolve_compose_file(file)?;
        //     run_docker_compose(&["down"], &compose_file)
        // },
        // ComposeAction::Convert { file } => {
        //     let compose_file = resolve_compose_file(file)?;
        //     run_kompose_convert(&["convert"], &compose_file)
        // }
    }
}

enum DockerRuntimes {
    Docker,
    Kaniko,
    Build
}

/// Represents the available subcommands within the `ShitAction` enum. This
/// enum uses the `Subcommand` derive macro to define various executable actions.
/// Each variant can optionally take arguments, which are defined using the `arg`
/// attribute macro.
///
/// # Variants
///
/// - `DoIt`:
///     Executes the `DoIt` action which optionally accepts the following parameter:
///
///     - `name`: An optional string parameter. It can be set using `-n` (short form)
///       or `--name` (long form) flag.
///
/// This design allows developers to extend functionality by adding additional
/// subcommands or parameters in the future. Note that other subcommands are currently
/// commented out and not active.
#[derive(Subcommand)]
pub enum ShitAction {
    // DoIt(DoItProps),
    DoIt {
        #[arg(short, long)]
        name: Option<String>,
    },
    Project {
        #[arg(short, long)]
        name: Option<String>,
        #[arg(short, long)]
        dry_run: Option<bool>,
        #[arg(short='s', long)]
        docker_runtime: Option<String>,
    }
    // Up {
    //     #[arg(short, long)]
    //     name: Option<String>,
    //     #[arg(short, long)]
    //     file: Option<String>,
    // },
    // Down {
    //     #[arg(short, long)]
    //     name: Option<String>,
    // },
}

#[derive(Subcommand)]
enum ClusterAction {
    Up {
        #[arg(short, long)]
        name: Option<String>,
        #[arg(short, long)]
        file: Option<String>,
    },
    Down {
        #[arg(short, long)]
        name: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum DashboardAction {
    Create {
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        query: String,
    },
    List,
}

#[derive(Subcommand)]
pub enum ComposeAction {
    Up {
        #[arg(short, long)]
        file: Option<String>,
        #[arg(short, long)]
        detatch: Option<String>,
    },
    Down {
        #[arg(short, long)]
        file: Option<String>,
    },
    Convert {
        #[arg(short, long)]
        file: Option<String>,
    }
}

pub async fn handle_compose(action: ComposeAction) -> anyhow::Result<()> {
    match action {
        ComposeAction::Up { file,detatch } => {
            // let start_time = Instant::now();
            let compose_file = resolve_compose_file(file)?;

            let result = run_docker_compose(&["up -d"], &compose_file).await;

            // if let Some(collector) = get_telemetry_collector() {
            //     let duration = start_time.elapsed().as_millis() as u64;
            //     let event = TelemetryEvent {
            //         command: "compose".to_string(),
            //         args: vec!["up".to_string()],
            //         duration_ms: duration,
            //         success: result.is_ok(),
            //         error_message: result.as_ref().err().map(|e| e.to_string()),
            //         timestamp: Utc::now(),
            //         tags: std::collections::HashMap::new(),
            //     };
            //     let _ = collector.record_event(event).await;
            // }

            result
        }
        ComposeAction::Down { file } => {
            // let start_time = Instant::now();
            let compose_file = resolve_compose_file(file)?;

            let result = run_docker_compose(&["down"], &compose_file).await;

            // if let Some(collector) = get_telemetry_collector() {
            //     let duration = start_time.elapsed().as_millis() as u64;
            //     let event = TelemetryEvent {
            //         command: "compose".to_string(),
            //         args: vec!["down".to_string()],
            //         duration_ms: duration,
            //         success: result.is_ok(),
            //         error_message: result.as_ref().err().map(|e| e.to_string()),
            //         timestamp: Utc::now(),
            //         tags: std::collections::HashMap::new(),
            //     };
            //     let _ = collector.record_event(event).await;
            // }

            result
        },
        ComposeAction::Convert { file } => {
            // let start_time = Instant::now();
            let compose_file = resolve_compose_file(file)?;

            let result = run_kompose_convert(&["convert"], &compose_file).await;

            // if let Some(collector) = get_telemetry_collector() {
            //     let duration = start_time.elapsed().as_millis() as u64;
            //     let event = TelemetryEvent {
            //         command: "kompose".to_string(),
            //         args: vec!["convert".to_string()],
            //         duration_ms: duration,
            //         success: result.is_ok(),
            //         error_message: result.as_ref().err().map(|e| e.to_string()),
            //         timestamp: Utc::now(),
            //         tags: std::collections::HashMap::new(),
            //     };
            //     let _ = collector.record_event(event).await;
            // }

            result
        }
    }
}
// kompose convert --file ~/projects/playground/manifests/dockers/compose.yaml
async fn run_kompose_convert(args: &[&str], compose_file: &str) -> anyhow::Result<()> {
    let mut cmd = Command::new("kompose");
    cmd.arg("--file").arg(compose_file);

    for arg in args {
        cmd.arg(arg);
    }

    let status = cmd.status()?;

    if !status.success() {
        anyhow::bail!("Kompose convert command failed");
    }

    Ok(())
}
async fn run_docker_compose(args: &[&str], compose_file: &str) -> anyhow::Result<()> {
    let mut cmd = Command::new("docker");
    cmd.arg("compose").arg("-f").arg(compose_file);

    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output()?;

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

pub async fn handle_dashboard(action: DashboardAction) -> anyhow::Result<()> {
    match action {
        DashboardAction::Create { name, query } => {
            // let start_time = Instant::now();

            let dashboard_template = serde_json::json!({
                "dashboard": {
                    "id": null,
                    "title": name,
                    "tags": ["custom", "cli-generated"],
                    "style": "dark",
                    "timezone": "browser",
                    "panels": [
                        {
                            "id": 1,
                            "title": format!("{} Panel", name),
                            "type": "timeseries",
                            "targets": [
                                {
                                    "datasource": {
                                        "type": "influxdb",
                                        "uid": "influxdb"
                                    },
                                    "query": query,
                                    "refId": "A"
                                }
                            ],
                            "fieldConfig": {
                                "defaults": {
                                    "color": {
                                        "mode": "palette-classic"
                                    }
                                }
                            },
                            "gridPos": {"h": 8, "w": 24, "x": 0, "y": 0}
                        }
                    ],
                    "time": {
                        "from": "now-1h",
                        "to": "now"
                    },
                    "refresh": "5s",
                    "schemaVersion": 27,
                    "version": 0
                }
            });

            let dashboard_path = format!("./scripts/nu/local-dev/grafana/provisioning/dashboards/{}.json", name);
            let dashboard_content = serde_json::to_string_pretty(&dashboard_template)?;

            let result = fs::write(&dashboard_path, dashboard_content);

            // if let Some(collector) = get_telemetry_collector() {
            //     let duration = start_time.elapsed().as_millis() as u64;
            //     let event = TelemetryEvent {
            //         command: "dashboard".to_string(),
            //         args: vec!["create".to_string(), name.clone()],
            //         duration_ms: duration,
            //         success: result.is_ok(),
            //         error_message: result.as_ref().err().map(|e| e.to_string()),
            //         timestamp: Utc::now(),
            //         tags: std::collections::HashMap::new(),
            //     };
            //     let _ = collector.record_event(event).await;
            // }

            match result {
                Ok(_) => {
                    println!("Dashboard '{}' created successfully at {}", name, dashboard_path);
                    Ok(())
                }
                Err(e) => anyhow::bail!("Failed to create dashboard: {}", e)
            }
        }
        DashboardAction::List => {
            // let start_time = Instant::now();
            let dashboard_dir = "./scripts/nu/local-dev/grafana/provisioning/dashboards";

            let result = fs::read_dir(dashboard_dir);

            // if let Some(collector) = get_telemetry_collector() {
            //     let duration = start_time.elapsed().as_millis() as u64;
            //     let event = TelemetryEvent {
            //         command: "dashboard".to_string(),
            //         args: vec!["list".to_string()],
            //         duration_ms: duration,
            //         success: result.is_ok(),
            //         error_message: result.as_ref().err().map(|e| e.to_string()),
            //         timestamp: Utc::now(),
            //         tags: std::collections::HashMap::new(),
            //     };
            //     let _ = collector.record_event(event).await;
            // }

            match result {
                Ok(entries) => {
                    println!("Available dashboards:");
                    for entry in entries {
                        if let Ok(entry) = entry {
                            if let Some(filename) = entry.file_name().to_str() {
                                if filename.ends_with(".json") {
                                    println!("  - {}", filename.trim_end_matches(".json"));
                                }
                            }
                        }
                    }
                    Ok(())
                }
                Err(e) => anyhow::bail!("Failed to list dashboards: {}", e)
            }
        }
    }
}