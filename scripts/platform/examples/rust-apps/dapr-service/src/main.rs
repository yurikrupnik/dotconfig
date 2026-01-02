//! Dapr-integrated Microservice
//!
//! Demonstrates Dapr integration patterns:
//! - State management (Redis)
//! - Pub/Sub messaging
//! - Service-to-service invocation
//! - Secrets management
//! - Actor pattern

use anyhow::{Context, Result};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use shared::{AgentMessage, Task, TaskResult, TaskStatus};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "dapr-service")]
#[command(about = "Dapr-integrated microservice")]
struct Args {
    /// Service port
    #[arg(long, env = "PORT", default_value = "3000")]
    port: u16,

    /// Dapr HTTP port
    #[arg(long, env = "DAPR_HTTP_PORT", default_value = "3500")]
    dapr_port: u16,

    /// State store name
    #[arg(long, env = "STATE_STORE", default_value = "statestore")]
    state_store: String,

    /// Pub/Sub component name
    #[arg(long, env = "PUBSUB_NAME", default_value = "pubsub")]
    pubsub_name: String,
}

/// Application state
struct AppState {
    dapr_client: DaprClient,
    config: Args,
    local_cache: RwLock<std::collections::HashMap<String, String>>,
}

/// Dapr HTTP client
struct DaprClient {
    http: reqwest::Client,
    base_url: String,
}

impl DaprClient {
    fn new(dapr_port: u16) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: format!("http://localhost:{dapr_port}"),
        }
    }

    /// Save state to Dapr state store
    async fn save_state<T: Serialize>(
        &self,
        store: &str,
        key: &str,
        value: &T,
    ) -> Result<()> {
        let url = format!("{}/v1.0/state/{store}", self.base_url);

        let state = vec![serde_json::json!({
            "key": key,
            "value": value,
        })];

        self.http
            .post(&url)
            .json(&state)
            .send()
            .await
            .context("Failed to save state")?
            .error_for_status()
            .context("Dapr state save failed")?;

        Ok(())
    }

    /// Get state from Dapr state store
    async fn get_state<T: for<'de> Deserialize<'de>>(
        &self,
        store: &str,
        key: &str,
    ) -> Result<Option<T>> {
        let url = format!("{}/v1.0/state/{store}/{key}", self.base_url);

        let response = self.http.get(&url).send().await.context("Failed to get state")?;

        if response.status() == StatusCode::NO_CONTENT {
            return Ok(None);
        }

        let value = response
            .json::<T>()
            .await
            .context("Failed to parse state")?;

        Ok(Some(value))
    }

    /// Delete state from Dapr state store
    async fn delete_state(&self, store: &str, key: &str) -> Result<()> {
        let url = format!("{}/v1.0/state/{store}/{key}", self.base_url);

        self.http
            .delete(&url)
            .send()
            .await
            .context("Failed to delete state")?
            .error_for_status()
            .context("Dapr state delete failed")?;

        Ok(())
    }

    /// Publish event to Dapr pub/sub
    async fn publish<T: Serialize>(
        &self,
        pubsub: &str,
        topic: &str,
        data: &T,
    ) -> Result<()> {
        let url = format!("{}/v1.0/publish/{pubsub}/{topic}", self.base_url);

        self.http
            .post(&url)
            .json(data)
            .send()
            .await
            .context("Failed to publish event")?
            .error_for_status()
            .context("Dapr publish failed")?;

        Ok(())
    }

    /// Invoke another Dapr service
    async fn invoke<T: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        app_id: &str,
        method: &str,
        data: Option<&T>,
    ) -> Result<R> {
        let url = format!("{}/v1.0/invoke/{app_id}/method/{method}", self.base_url);

        let mut request = self.http.post(&url);
        if let Some(data) = data {
            request = request.json(data);
        }

        let response = request
            .send()
            .await
            .context("Failed to invoke service")?
            .error_for_status()
            .context("Service invocation failed")?
            .json::<R>()
            .await
            .context("Failed to parse response")?;

        Ok(response)
    }

    /// Get secret from Dapr secret store
    async fn get_secret(&self, store: &str, key: &str) -> Result<String> {
        let url = format!("{}/v1.0/secrets/{store}/{key}", self.base_url);

        let response = self
            .http
            .get(&url)
            .send()
            .await
            .context("Failed to get secret")?
            .error_for_status()
            .context("Dapr secret fetch failed")?
            .json::<std::collections::HashMap<String, String>>()
            .await
            .context("Failed to parse secret")?;

        response
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Secret key not found"))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("dapr_service=info".parse()?)
        )
        .json()
        .init();

    let args = Args::parse();
    info!("Starting Dapr service on port {}", args.port);

    let state = Arc::new(AppState {
        dapr_client: DaprClient::new(args.dapr_port),
        config: Args::parse(),
        local_cache: RwLock::new(std::collections::HashMap::new()),
    });

    // Build router
    let app = Router::new()
        // Health endpoints
        .route("/health", get(health))
        .route("/healthz", get(health))
        // State management
        .route("/state/:key", get(get_state).post(save_state).delete(delete_state))
        // Task management
        .route("/tasks", post(create_task))
        .route("/tasks/:id", get(get_task))
        .route("/tasks/:id/complete", post(complete_task))
        // Pub/Sub subscription endpoints (Dapr calls these)
        .route("/dapr/subscribe", get(subscribe))
        .route("/events/task-created", post(handle_task_created))
        .route("/events/task-completed", post(handle_task_completed))
        // Service invocation
        .route("/invoke/:service/:method", post(invoke_service))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    // Run server
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port)).await?;
    info!("Listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Health check endpoint
async fn health() -> &'static str {
    "OK"
}

/// Get state by key
async fn get_state(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state
        .dapr_client
        .get_state::<serde_json::Value>(&state.config.state_store, &key)
        .await
    {
        Ok(Some(value)) => Ok(Json(value)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to get state: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Save state
async fn save_state(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(value): Json<serde_json::Value>,
) -> Result<StatusCode, StatusCode> {
    state
        .dapr_client
        .save_state(&state.config.state_store, &key, &value)
        .await
        .map_err(|e| {
            error!("Failed to save state: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::CREATED)
}

/// Delete state
async fn delete_state(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state
        .dapr_client
        .delete_state(&state.config.state_store, &key)
        .await
        .map_err(|e| {
            error!("Failed to delete state: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::NO_CONTENT)
}

/// Create a new task
#[derive(Deserialize)]
struct CreateTaskRequest {
    task_type: String,
    payload: serde_json::Value,
    #[serde(default)]
    priority: Option<u8>,
}

async fn create_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<Json<Task>, StatusCode> {
    let mut task = Task::new(req.task_type, req.payload);
    if let Some(priority) = req.priority {
        task = task.with_priority(priority);
    }

    // Save task to state store
    let key = format!("task:{}", task.id);
    state
        .dapr_client
        .save_state(&state.config.state_store, &key, &task)
        .await
        .map_err(|e| {
            error!("Failed to save task: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Publish task created event
    state
        .dapr_client
        .publish(&state.config.pubsub_name, "task-created", &task)
        .await
        .map_err(|e| {
            error!("Failed to publish task created event: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!(task_id = %task.id, "Task created");
    Ok(Json(task))
}

/// Get task by ID
async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Task>, StatusCode> {
    let key = format!("task:{id}");

    match state
        .dapr_client
        .get_state::<Task>(&state.config.state_store, &key)
        .await
    {
        Ok(Some(task)) => Ok(Json(task)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to get task: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Complete a task
#[derive(Deserialize)]
struct CompleteTaskRequest {
    result: serde_json::Value,
}

async fn complete_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<CompleteTaskRequest>,
) -> Result<Json<TaskResult>, StatusCode> {
    let result = TaskResult {
        task_id: id,
        status: TaskStatus::Completed,
        result: Some(req.result),
        error: None,
        duration_ms: 0,
        completed_at: chrono::Utc::now(),
    };

    // Save result
    let key = format!("task:result:{id}");
    state
        .dapr_client
        .save_state(&state.config.state_store, &key, &result)
        .await
        .map_err(|e| {
            error!("Failed to save task result: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Publish completion event
    state
        .dapr_client
        .publish(&state.config.pubsub_name, "task-completed", &result)
        .await
        .map_err(|e| {
            error!("Failed to publish task completed event: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!(task_id = %id, "Task completed");
    Ok(Json(result))
}

/// Dapr subscription configuration
#[derive(Serialize)]
struct Subscription {
    pubsubname: String,
    topic: String,
    route: String,
}

async fn subscribe(State(state): State<Arc<AppState>>) -> Json<Vec<Subscription>> {
    Json(vec![
        Subscription {
            pubsubname: state.config.pubsub_name.clone(),
            topic: "task-created".to_string(),
            route: "/events/task-created".to_string(),
        },
        Subscription {
            pubsubname: state.config.pubsub_name.clone(),
            topic: "task-completed".to_string(),
            route: "/events/task-completed".to_string(),
        },
    ])
}

/// Handle task created event
#[derive(Deserialize)]
struct CloudEvent<T> {
    data: T,
}

async fn handle_task_created(
    Json(event): Json<CloudEvent<Task>>,
) -> StatusCode {
    info!(task_id = %event.data.id, "Received task created event");
    // Process the event (e.g., queue for processing, notify, etc.)
    StatusCode::OK
}

/// Handle task completed event
async fn handle_task_completed(
    Json(event): Json<CloudEvent<TaskResult>>,
) -> StatusCode {
    info!(task_id = %event.data.task_id, "Received task completed event");
    // Process the event (e.g., update UI, trigger downstream, etc.)
    StatusCode::OK
}

/// Invoke another Dapr service
#[derive(Deserialize)]
struct InvokeRequest {
    data: Option<serde_json::Value>,
}

async fn invoke_service(
    State(state): State<Arc<AppState>>,
    Path((service, method)): Path<(String, String)>,
    Json(req): Json<InvokeRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let result = state
        .dapr_client
        .invoke::<serde_json::Value, serde_json::Value>(&service, &method, req.data.as_ref())
        .await
        .map_err(|e| {
            error!("Failed to invoke service: {}", e);
            StatusCode::BAD_GATEWAY
        })?;

    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dapr_client_urls() {
        let client = DaprClient::new(3500);
        assert_eq!(client.base_url, "http://localhost:3500");
    }
}
