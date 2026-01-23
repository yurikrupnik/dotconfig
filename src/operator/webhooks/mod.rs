//! Admission Webhooks for Platform Operator
//!
//! This module provides:
//! - Mutating webhook for ownership injection
//! - Validating webhook for RBAC enforcement

pub mod authz;
pub mod ownership;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use kube::core::admission::{AdmissionRequest, AdmissionResponse, AdmissionReview};
use kube::core::DynamicObject;
use std::sync::Arc;
use tracing::{error, info};

use crate::operator::Context;

/// Create the webhook router
pub fn create_webhook_router(ctx: Arc<Context>) -> Router {
    Router::new()
        .route("/mutate/ownership", post(mutate_ownership))
        .route("/validate/authz", post(validate_authz))
        .route("/healthz", post(healthz))
        .with_state(ctx)
}

/// Health check endpoint
async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}

/// Mutating webhook for ownership injection
async fn mutate_ownership(
    State(ctx): State<Arc<Context>>,
    Json(review): Json<AdmissionReview<DynamicObject>>,
) -> impl IntoResponse {
    let request: AdmissionRequest<DynamicObject> = match review.try_into() {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to parse admission request: {}", e);
            return Json(AdmissionResponse::invalid(format!("Invalid request: {}", e)).into_review());
        }
    };

    info!(
        "Mutating webhook called for {}/{} by {}",
        request.namespace.as_deref().unwrap_or("cluster"),
        &request.name,
        request.user_info.username.as_deref().unwrap_or("unknown")
    );

    let response = ownership::mutate_ownership(&request, &ctx).await;
    Json(response.into_review())
}

/// Validating webhook for RBAC enforcement
async fn validate_authz(
    State(ctx): State<Arc<Context>>,
    Json(review): Json<AdmissionReview<DynamicObject>>,
) -> impl IntoResponse {
    let request: AdmissionRequest<DynamicObject> = match review.try_into() {
        Ok(req) => req,
        Err(e) => {
            error!("Failed to parse admission request: {}", e);
            return Json(AdmissionResponse::invalid(format!("Invalid request: {}", e)).into_review());
        }
    };

    info!(
        "Validating webhook called for {}/{} by {}",
        request.namespace.as_deref().unwrap_or("cluster"),
        &request.name,
        request.user_info.username.as_deref().unwrap_or("unknown")
    );

    let response = authz::validate_authz(&request, &ctx).await;
    Json(response.into_review())
}
