//! # Server State
//!
//! This module defines the application state shared across HTTP handlers.

use sqlx::PgPool;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use tokio::sync::broadcast;

use crate::database::cache::DashboardCache;
use crate::events::HazardEvent;
use crate::settings::ServerConfig;

/// Application state shared across all HTTP handlers.
#[derive(Clone)]
pub struct AppState {
    /// The PostgreSQL connection pool.
    pub db_pool: PgPool,
    /// The current application version string.
    pub version: String,
    /// Shared server configuration.
    pub config: Arc<ServerConfig>,
    /// TTL-cached `DashboardRepository` reads, shared by the JSON API and
    /// HTMX SSR partials.
    pub dashboard_cache: DashboardCache,
    /// Optional Prometheus configuration for scraping.
    pub prometheus_config: Option<crate::settings::PrometheusConfig>,
    /// Optional OTLP/Prometheus Remote Write config for Grafana Cloud.
    pub grafana_cloud_prometheus_config: Option<crate::settings::GrafanaCloudPrometheusConfig>,
    /// Fan-out sender for hazard events. `/api/events/hazards` subscribes to
    /// it per-connection; `/internal/events` and (with `pg-listen`) the
    /// Postgres listener publish to it.
    pub events_tx: broadcast::Sender<HazardEvent>,
    /// Shared secret required on `X-Internal-Token` for `/internal/events`.
    /// Loaded from the `INTERNAL_EVENT_TOKEN` env var only, never committed
    /// config.
    pub internal_event_token: Arc<str>,
    /// Count of currently-connected SSE subscribers, used to enforce
    /// `ServerConfig.max_hazard_subscribers`.
    pub hazard_subscriber_count: Arc<AtomicUsize>,
    /// Flips to `true` when the server starts shutting down. SSE handlers
    /// watch this to end their (otherwise infinite) streams promptly, so
    /// `axum::serve`'s graceful shutdown — which waits for open connections
    /// to finish on their own — doesn't hang on clients that never disconnect.
    pub shutdown: tokio::sync::watch::Receiver<bool>,
}

impl AppState {
    /// Creates a new application state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db_pool: PgPool,
        config: ServerConfig,
        version: String,
        prometheus_config: Option<crate::settings::PrometheusConfig>,
        grafana_cloud_prometheus_config: Option<crate::settings::GrafanaCloudPrometheusConfig>,
        events_tx: broadcast::Sender<HazardEvent>,
        internal_event_token: Arc<str>,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        let dashboard_cache = DashboardCache::new(&config.cache);
        Self {
            db_pool,
            version,
            config: Arc::new(config),
            dashboard_cache,
            prometheus_config,
            grafana_cloud_prometheus_config,
            events_tx,
            internal_event_token,
            hazard_subscriber_count: Arc::new(AtomicUsize::new(0)),
            shutdown,
        }
    }
}
