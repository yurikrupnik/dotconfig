use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    response::{Html, IntoResponse, Response},
};
use leptos::prelude::*;

use crate::App;

pub async fn leptos_routes_handler(
    State(options): State<LeptosOptions>,
    req: Request<Body>,
) -> Response {
    let handler = leptos_axum::render_app_to_stream_with_context(
        move || {
            provide_context(options.clone());
        },
        App,
    );

    handler(req).await.into_response()
}

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

pub async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Html("Not Found"))
}

#[derive(Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
}
