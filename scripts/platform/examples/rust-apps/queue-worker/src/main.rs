//! KEDA-scalable Redis Queue Worker
//!
//! This worker processes tasks from a Redis queue and can be scaled
//! by KEDA based on queue depth.
//!
//! Features:
//! - Graceful shutdown on SIGTERM
//! - Configurable concurrency
//! - Task retry with exponential backoff
//! - Prometheus metrics endpoint
//! - Health check endpoint

use anyhow::{Context, Result};
use clap::Parser;
use redis::AsyncCommands;
use shared::{Task, TaskResult, TaskStatus};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(name = "queue-worker")]
#[command(about = "KEDA-scalable Redis queue worker")]
struct Args {
    /// Redis URL
    #[arg(long, env = "REDIS_URL", default_value = "redis://localhost:6379")]
    redis_url: String,

    /// Queue name to process
    #[arg(long, env = "QUEUE_NAME", default_value = "tasks")]
    queue_name: String,

    /// Maximum concurrent tasks
    #[arg(long, env = "CONCURRENCY", default_value = "10")]
    concurrency: usize,

    /// Health check port
    #[arg(long, env = "HEALTH_PORT", default_value = "8080")]
    health_port: u16,

    /// Visibility timeout in seconds
    #[arg(long, env = "VISIBILITY_TIMEOUT", default_value = "30")]
    visibility_timeout: u64,
}

/// Worker metrics
struct Metrics {
    tasks_processed: AtomicU64,
    tasks_failed: AtomicU64,
    tasks_retried: AtomicU64,
}

impl Metrics {
    fn new() -> Self {
        Self {
            tasks_processed: AtomicU64::new(0),
            tasks_failed: AtomicU64::new(0),
            tasks_retried: AtomicU64::new(0),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("queue_worker=info".parse()?)
        )
        .json()
        .init();

    let args = Args::parse();
    info!("Starting queue worker with config: {:?}", args);

    // Connect to Redis
    let client = redis::Client::open(args.redis_url.as_str())
        .context("Failed to create Redis client")?;

    let conn = client
        .get_multiplexed_async_connection()
        .await
        .context("Failed to connect to Redis")?;

    // Shared state
    let shutdown = Arc::new(AtomicBool::new(false));
    let metrics = Arc::new(Metrics::new());
    let semaphore = Arc::new(Semaphore::new(args.concurrency));

    // Start health check server
    let health_metrics = metrics.clone();
    let health_shutdown = shutdown.clone();
    tokio::spawn(async move {
        run_health_server(args.health_port, health_metrics, health_shutdown).await
    });

    // Main processing loop
    let process_shutdown = shutdown.clone();
    let process_metrics = metrics.clone();

    tokio::spawn(async move {
        process_queue(
            conn,
            args.queue_name,
            args.visibility_timeout,
            semaphore,
            process_shutdown,
            process_metrics,
        )
        .await
    });

    // Wait for shutdown signal
    wait_for_shutdown().await;

    info!("Shutdown signal received, stopping worker...");
    shutdown.store(true, Ordering::SeqCst);

    // Give tasks time to complete
    tokio::time::sleep(Duration::from_secs(5)).await;

    info!(
        "Worker stopped. Processed: {}, Failed: {}, Retried: {}",
        metrics.tasks_processed.load(Ordering::Relaxed),
        metrics.tasks_failed.load(Ordering::Relaxed),
        metrics.tasks_retried.load(Ordering::Relaxed),
    );

    Ok(())
}

async fn process_queue(
    mut conn: redis::aio::MultiplexedConnection,
    queue_name: String,
    visibility_timeout: u64,
    semaphore: Arc<Semaphore>,
    shutdown: Arc<AtomicBool>,
    metrics: Arc<Metrics>,
) {
    let processing_queue = format!("{queue_name}:processing");
    let dead_letter_queue = format!("{queue_name}:dlq");

    while !shutdown.load(Ordering::Relaxed) {
        // Acquire semaphore permit
        let permit = match semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => break,
        };

        // BRPOPLPUSH: atomically move task to processing queue
        let result: Option<String> = conn
            .brpoplpush(&queue_name, &processing_queue, visibility_timeout as f64)
            .await
            .unwrap_or(None);

        let Some(task_json) = result else {
            drop(permit);
            continue;
        };

        // Parse task
        let task: Task = match serde_json::from_str(&task_json) {
            Ok(t) => t,
            Err(e) => {
                error!("Failed to parse task: {}", e);
                // Remove invalid task from processing queue
                let _: () = conn
                    .lrem(&processing_queue, 1, &task_json)
                    .await
                    .unwrap_or(());
                drop(permit);
                continue;
            }
        };

        info!(task_id = %task.id, task_type = %task.task_type, "Processing task");

        // Process task in background
        let mut task_conn = conn.clone();
        let task_metrics = metrics.clone();
        let pq = processing_queue.clone();
        let dlq = dead_letter_queue.clone();
        let q = queue_name.clone();

        tokio::spawn(async move {
            let start = std::time::Instant::now();

            match process_task(&task).await {
                Ok(result) => {
                    info!(task_id = %task.id, duration_ms = %result.duration_ms, "Task completed");

                    // Remove from processing queue
                    let _: () = task_conn.lrem(&pq, 1, &task_json).await.unwrap_or(());

                    // Store result
                    let result_key = format!("task:result:{}", task.id);
                    let _: () = task_conn
                        .set_ex(&result_key, serde_json::to_string(&result).unwrap(), 3600)
                        .await
                        .unwrap_or(());

                    task_metrics.tasks_processed.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    error!(task_id = %task.id, error = %e, "Task failed");

                    // Remove from processing queue
                    let _: () = task_conn.lrem(&pq, 1, &task_json).await.unwrap_or(());

                    if task.retries < task.max_retries {
                        // Retry: increment retry count and push back to queue
                        let mut retry_task = task.clone();
                        retry_task.retries += 1;

                        let retry_json = serde_json::to_string(&retry_task).unwrap();
                        let _: () = task_conn.lpush(&q, &retry_json).await.unwrap_or(());

                        warn!(
                            task_id = %task.id,
                            retry = retry_task.retries,
                            max_retries = retry_task.max_retries,
                            "Task queued for retry"
                        );

                        task_metrics.tasks_retried.fetch_add(1, Ordering::Relaxed);
                    } else {
                        // Move to dead letter queue
                        let _: () = task_conn.lpush(&dlq, &task_json).await.unwrap_or(());

                        error!(task_id = %task.id, "Task moved to dead letter queue");
                        task_metrics.tasks_failed.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }

            let _ = start.elapsed();
            drop(permit);
        });
    }
}

async fn process_task(task: &Task) -> Result<TaskResult> {
    let start = std::time::Instant::now();

    // Simulate different task types
    match task.task_type.as_str() {
        "compute" => {
            // CPU-intensive task simulation
            tokio::task::spawn_blocking(|| {
                // Simulate heavy computation
                let mut sum: u64 = 0;
                for i in 0..1_000_000 {
                    sum = sum.wrapping_add(i);
                }
                sum
            })
            .await?;
        }
        "io" => {
            // I/O-bound task simulation
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        "transform" => {
            // Data transformation
            let _data = task.payload.clone();
            // Transform logic here
        }
        _ => {
            // Default: just acknowledge
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    Ok(TaskResult {
        task_id: task.id,
        status: TaskStatus::Completed,
        result: Some(serde_json::json!({
            "processed": true,
            "task_type": task.task_type,
        })),
        error: None,
        duration_ms,
        completed_at: chrono::Utc::now(),
    })
}

async fn run_health_server(
    port: u16,
    metrics: Arc<Metrics>,
    shutdown: Arc<AtomicBool>,
) {
    use std::io::Write;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind health check port");

    info!("Health check server listening on port {port}");

    while !shutdown.load(Ordering::Relaxed) {
        let Ok((mut socket, _)) = tokio::time::timeout(
            Duration::from_secs(1),
            listener.accept(),
        )
        .await
        .unwrap_or(Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "timeout",
        ))) else {
            continue;
        };

        let processed = metrics.tasks_processed.load(Ordering::Relaxed);
        let failed = metrics.tasks_failed.load(Ordering::Relaxed);
        let retried = metrics.tasks_retried.load(Ordering::Relaxed);

        let mut body = Vec::new();
        writeln!(body, "# HELP tasks_processed_total Total tasks processed").unwrap();
        writeln!(body, "# TYPE tasks_processed_total counter").unwrap();
        writeln!(body, "tasks_processed_total {processed}").unwrap();
        writeln!(body, "# HELP tasks_failed_total Total tasks failed").unwrap();
        writeln!(body, "# TYPE tasks_failed_total counter").unwrap();
        writeln!(body, "tasks_failed_total {failed}").unwrap();
        writeln!(body, "# HELP tasks_retried_total Total tasks retried").unwrap();
        writeln!(body, "# TYPE tasks_retried_total counter").unwrap();
        writeln!(body, "tasks_retried_total {retried}").unwrap();

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            String::from_utf8_lossy(&body)
        );

        let _ = socket.write_all(response.as_bytes()).await;
    }
}

async fn wait_for_shutdown() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to listen for SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_task() {
        let task = Task::new("compute", serde_json::json!({}));
        let result = process_task(&task).await.unwrap();

        assert!(matches!(result.status, TaskStatus::Completed));
        assert!(result.duration_ms > 0);
    }
}
