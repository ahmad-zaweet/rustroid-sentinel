//! # Server Router Configuration
//!
//! This module handles router setup and configuration.

use axum::{Router, extract::State, http::StatusCode, middleware, routing::get};
use axum_governor::GovernorLayer;
use axum_helmet::Helmet;
use real::RealIpLayer;
use std::time::Duration;
use tower_http::{
    compression::CompressionLayer, limit::RequestBodyLimitLayer, services::ServeDir,
    timeout::TimeoutLayer, trace::TraceLayer,
};

use crate::api;

use super::middleware::{
    api_cache_control, cors_middleware, health_check_handler, metrics_middleware_wrapper,
    static_cache_control,
};
use super::state::AppState;

/// Builds the main application router with all middleware and routes.
pub fn build_router(state: AppState, timeout: Duration) -> Router {
    // API router with cache control
    let api_router = Router::new()
        .merge(api::routes::api_router())
        .layer(middleware::from_fn(api_cache_control))
        .with_state(state.clone());

    // Static file service
    let static_service = tower::ServiceBuilder::new()
        .layer(middleware::from_fn(static_cache_control))
        .service(
            ServeDir::new("static")
                .append_index_html_on_directories(true)
                .precompressed_gzip(),
        );

    // Build CSP policy
    let csp = helmet_core::ContentSecurityPolicy::new()
        .script_src(vec![
            "'self'",
            "'unsafe-inline'",
            "https://cdn.tailwindcss.com",
            "https://cdn.jsdelivr.net",
            "https://unpkg.com/lucide@latest",
            "https://unpkg.com/htmx.org@2.0.0",
        ])
        .style_src(vec![
            "'self'",
            "'unsafe-inline'",
            "https://cdn.tailwindcss.com",
            "https://fonts.googleapis.com",
        ])
        .font_src(vec![
            "'self'",
            "https://fonts.googleapis.com",
            "https://fonts.gstatic.com",
        ])
        .img_src(vec!["'self'", "data:", "https:"])
        .connect_src(vec!["'self'", "ws:", "wss:"]);

    let helmet = Helmet::new().add(csp);

    Router::new()
        .layer(RealIpLayer::default())
        .layer(GovernorLayer::default())
        .route("/", get(crate::api::handlers::render_dashboard))
        .route("/health", get(health_check_handler))
        .nest("/api", api_router)
        .nest("/dashboard", crate::api::routes::dashboard_router())
        .route("/metrics", get(crate::metrics::get_metrics))
        .route(
            "/api/metrics/summary",
            get(move |state: State<AppState>| async move {
                crate::metrics::get_metrics_summary(
                    &state.prometheus_config,
                    &state.grafana_cloud_prometheus_config,
                    &state.db_pool,
                )
                .await
            }),
        )
        .fallback_service(static_service)
        // Security: Limit request body size to 1MB to prevent DoS
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .layer(
            helmet
                .into_layer()
                .map_err(|e| {
                    std::io::Error::other(
                        format!("Failed to initialize Axum-Helmet layer: {}", e),
                    )
                })
                .expect("Failed to initialize Helmet layer"),
        )
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new().gzip(true))
        .layer(middleware::from_fn(metrics_middleware_wrapper))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            cors_middleware,
        ))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            timeout,
        ))
        .with_state(state)
}
