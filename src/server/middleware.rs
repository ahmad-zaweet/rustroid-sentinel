//! # Server Middleware
//!
//! This module provides HTTP middleware components for the server.

use axum::{
    extract::{Request, State},
    http::{
        StatusCode,
        header::{CACHE_CONTROL, HeaderValue},
    },
    middleware::Next,
    response::{IntoResponse, Response},
};

use super::state::AppState;

/// Middleware to add cache control headers for API responses.
pub async fn api_cache_control(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    response
}

/// Middleware to add cache control headers for static assets.
pub async fn static_cache_control(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    response
}

/// CORS middleware for restricting cross-origin requests.
pub async fn cors_middleware(
    State(_state): axum::extract::State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let origin = req.headers().get("origin").cloned();

    // Restrictive CORS check
    let is_allowed = match &origin {
        Some(val) => {
            let val_str = val.to_str().unwrap_or("");
            val_str == "http://localhost:8000" || val_str == "http://127.0.0.1:8000"
        }
        None => true,
    };

    if !is_allowed {
        return (StatusCode::FORBIDDEN, "CORS origin not allowed").into_response();
    }

    let mut response = next.run(req).await;
    if let Some(origin_val) = origin {
        response
            .headers_mut()
            .insert("access-control-allow-origin", origin_val);
        response.headers_mut().insert(
            "access-control-allow-methods",
            HeaderValue::from_static("GET"),
        );
    }
    response
}

/// Wrapper for metrics middleware.
pub async fn metrics_middleware_wrapper(req: Request, next: Next) -> Response {
    crate::metrics::metrics_middleware(req, next).await
}

/// Health check handler.
pub async fn health_check_handler(
    State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let db_status = if sqlx::query("SELECT 1")
        .execute(&state.db_pool)
        .await
        .is_ok()
    {
        "up"
    } else {
        "down"
    };

    let status_code = if db_status == "up" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let body = serde_json::json!({
        "status": "ok",
        "version": state.version,
        "database": db_status
    });

    (status_code, axum::Json(body))
}
