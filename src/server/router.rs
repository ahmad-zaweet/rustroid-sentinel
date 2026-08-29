//! # Server Router Configuration
//!
//! This module handles router setup and configuration.

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    middleware,
    routing::{get, post},
};
use axum_governor::{GovernorConfigBuilder, GovernorLayer, Quota, extractor::PeerIp};
use axum_helmet::Helmet;
use real::RealIpLayer;
use std::num::NonZeroU32;
use std::time::Duration;
use tower_http::{
    compression::{
        CompressionLayer,
        predicate::{DefaultPredicate, NotForContentType, Predicate},
    },
    limit::RequestBodyLimitLayer,
    services::ServeDir,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use crate::api::handlers::internal_events::MAX_BODY_BYTES as INTERNAL_EVENTS_MAX_BODY_BYTES;

use crate::api;
use crate::settings::ServerConfig;

use super::middleware::{
    api_cache_control, cors_middleware, health_check_handler, metrics_middleware_wrapper,
    static_cache_control,
};
use super::state::AppState;

/// Builds the main application router with all middleware and routes.
#[allow(clippy::too_many_lines)]
pub fn build_router(state: AppState, timeout: Duration, server_config: ServerConfig) -> Router {
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
            "https://unpkg.com/lucide@0.577.0",
            "https://unpkg.com/htmx.org@2.0.10",
            "https://unpkg.com/htmx-ext-sse@2.2.2",
            "https://cdn.jsdelivr.net/npm/chart.js@4.5.1/dist/chart.umd.min.js.map",
        ])
        .style_src(vec![
            "'self'",
            "'unsafe-inline'",
            "https://cdn.tailwindcss.com",
            "https://fonts.googleapis.com",
            "https://cdn.jsdelivr.net/npm/flatpickr/dist/flatpickr.min.css",
        ])
        .font_src(vec![
            "'self'",
            "https://fonts.googleapis.com",
            "https://fonts.gstatic.com",
        ])
        .img_src(vec!["'self'", "data:", "https:"])
        .connect_src(vec!["'self'", "ws:", "wss:"]);

    let helmet = Helmet::new().add(csp);

    let cfg = GovernorConfigBuilder::default()
        .with_extractor(PeerIp::default())
        .expect_connect_info()
        .quota_default(Quota::requests_per_minute(
            NonZeroU32::new(server_config.rate_limit_requests as u32)
                .expect("rate_limit_requests must be non-zero"),
        ))
        .finish()
        .unwrap();

    // Tighter quota for the internal ingest webhook: it has a single trusted
    // caller (the ETL job), not public traffic.
    let internal_events_cfg = GovernorConfigBuilder::default()
        .with_extractor(PeerIp::default())
        .expect_connect_info()
        .quota_default(Quota::requests_per_minute(
            NonZeroU32::new(server_config.internal_event_rate_limit_requests as u32)
                .expect("internal_event_rate_limit_requests must be non-zero"),
        ))
        .finish()
        .unwrap();

    // Registered outside the `/api` cache-control layer and not linked from
    // the dashboard: this is a webhook, not a public endpoint.
    let internal_events_router = Router::new()
        .route(
            "/internal/events",
            post(crate::api::handlers::ingest_events),
        )
        .route_layer(RequestBodyLimitLayer::new(INTERNAL_EVENTS_MAX_BODY_BYTES))
        .route_layer(GovernorLayer::new(internal_events_cfg))
        .with_state(state.clone());

    Router::new()
        .layer(RealIpLayer::default())
        .layer(GovernorLayer::new(cfg))
        .route("/", get(crate::api::handlers::render_dashboard))
        .route("/health", get(health_check_handler))
        .merge(internal_events_router)
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
                    std::io::Error::other(format!("Failed to initialize Axum-Helmet layer: {}", e))
                })
                .expect("Failed to initialize Helmet layer"),
        )
        .layer(TraceLayer::new_for_http())
        // `text/event-stream` is excluded: gzip buffers output until a block
        // is full, which stalls the hazard SSE stream instead of flushing
        // events as they're published (breaks badly behind Render's proxy).
        .layer(CompressionLayer::new().gzip(true).compress_when(
            DefaultPredicate::new().and(NotForContentType::new("text/event-stream")),
        ))
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
