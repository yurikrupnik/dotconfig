//! Shared API core logic
//!
//! This crate contains the business logic that can be used by:
//! - Tauri desktop apps (via direct function calls)
//! - HTTP API server (for web/K8s deployments)

use serde::{Deserialize, Serialize};

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize, Serialize)]
pub struct GreetRequest {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GreetResponse {
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

// ============================================================================
// Core Business Logic
// ============================================================================

/// Greet a user by name
///
/// This is the core business logic that can be called from:
/// - Tauri commands
/// - HTTP handlers
pub fn greet(name: &str) -> String {
    if name.is_empty() {
        "Hello, stranger! Please tell me your name.".to_string()
    } else {
        format!("Hello, {}! You've been greeted from Rust!", name)
    }
}

/// Get health status
pub fn health() -> HealthResponse {
    HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet_with_name() {
        let result = greet("Alice");
        assert_eq!(result, "Hello, Alice! You've been greeted from Rust!");
    }

    #[test]
    fn test_greet_empty_name() {
        let result = greet("");
        assert_eq!(result, "Hello, stranger! Please tell me your name.");
    }

    #[test]
    fn test_health() {
        let result = health();
        assert_eq!(result.status, "ok");
    }
}
