//! HTTP API Server
//!
//! Standalone server that exposes the API core logic via HTTP.
//! Can be deployed to Kubernetes, Docker, or run locally.

use api_core::{greet, health, GreetRequest, GreetResponse, HealthResponse};
use axum::{routing::{get, post}, Json, Router};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ============================================================================
// Handlers
// ============================================================================

async fn health_handler() -> Json<HealthResponse> {
    Json(health())
}

async fn greet_handler(Json(req): Json<GreetRequest>) -> Json<GreetResponse> {
    Json(GreetResponse {
        message: greet(&req.name),
    })
}

// ============================================================================
// Router
// ============================================================================

fn create_router() -> Router {
    // CORS configuration - permissive for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_handler))
        .route("/api/greet", post(greet_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "api_server=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = create_router();

    // Get port from environment or default to 3000
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("API server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test::TestServer;

    #[tokio::test]
    async fn test_health_endpoint() {
        let server = TestServer::new(create_router()).unwrap();
        let response = server.get("/health").await;

        response.assert_status_ok();
        let body: HealthResponse = response.json();
        assert_eq!(body.status, "ok");
    }

    #[tokio::test]
    async fn test_greet_endpoint() {
        let server = TestServer::new(create_router()).unwrap();
        let response = server
            .post("/api/greet")
            .json(&GreetRequest { name: "Test".to_string() })
            .await;

        response.assert_status_ok();
        let body: GreetResponse = response.json();
        assert!(body.message.contains("Test"));
    }
}
