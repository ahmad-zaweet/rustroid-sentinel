//! # API Routes
//!
//! This module defines the router configuration for the REST API endpoints.
//! It uses [`axum`]'s router composition to organize endpoints by resource.

use super::handlers;
use crate::server::AppState;
use axum::{Router, routing::get};

/// Creates the API router with all REST endpoints.
///
/// This router includes health checks, statistics, velocity time-series,
/// paginated approaches, and ETL run history. All routes are relative to
/// the `/api` prefix when mounted in the main application.
///
/// # Routes
///
/// | Method | Endpoint        | Handler      | Description                        |
/// |--------|-----------------|--------------|------------------------------------|
/// | GET    | `/health`       | [`handlers::health`]     | Service health check with DB status  |
/// | GET    | `/stats`        | [`handlers::stats`]      | Dashboard statistics summary         |
/// | GET    | `/velocity`     | [`handlers::velocity`]   | Velocity time-series with filtering  |
/// | GET    | `/approaches`   | [`handlers::approaches`] | Paginated approach records           |
/// | GET    | `/etl-runs`     | [`handlers::etl_runs`]   | Recent ETL job history               |
///
/// # Type Parameters
///
/// * `AppState` - The shared application state containing database pool, config, etc.
///
/// # Examples
///
/// ```rust,no_run
/// use rustroid_sentinel::api::routes::api_router;
/// use rustroid_sentinel::server::AppState;
/// use axum::Router;
///
/// # async fn example(state: AppState) {
/// let api = api_router();
/// let app: Router<AppState> = Router::new()
///     .nest("/api", api)
///     .with_state(state);
/// # }
/// ```
pub fn api_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/stats", get(handlers::stats))
        .route("/velocity", get(handlers::velocity))
        .route("/approaches", get(handlers::approaches))
        .route("/etl-runs", get(handlers::etl_runs))
}

/// Creates the dashboard router with HTMX partial endpoints.
///
/// This router includes SSR endpoints for htmx-powered partial page updates.
/// Routes are relative to the `/dashboard` prefix.
///
/// # Routes
///
/// | Method | Endpoint       | Handler              | Description                        |
/// |--------|----------------|----------------------|------------------------------------|
/// | GET    | `/table`       | [`handlers::dashboard_table`] | HTMX partial for approaches table |
/// | GET    | `/etl-runs`    | [`handlers::dashboard_etl_runs`] | HTMX partial for ETL runs with pagination |
/// | GET    | `/velocity`    | [`handlers::refresh_velocity_chart`] | HTMX partial for velocity chart refresh |
/// | GET    | `/metrics`     | [`handlers::refresh_metrics`] | HTMX partial for system metrics refresh |
pub fn dashboard_router() -> Router<AppState> {
    Router::new()
        .route("/table", get(handlers::dashboard_table))
        .route("/etl-runs", get(handlers::dashboard_etl_runs))
        .route("/velocity", get(handlers::refresh_velocity_chart))
        .route("/metrics", get(handlers::refresh_metrics))
}
