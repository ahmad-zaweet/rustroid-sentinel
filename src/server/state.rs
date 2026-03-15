//! # Server State
//!
//! This module defines the application state shared across HTTP handlers.

use sqlx::PgPool;
use std::sync::Arc;

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
    /// Optional Prometheus configuration for scraping.
    pub prometheus_config: Option<crate::settings::PrometheusConfig>,
    /// Optional OTLP/Prometheus Remote Write config for Grafana Cloud.
    pub grafana_cloud_prometheus_config: Option<crate::settings::GrafanaCloudPrometheusConfig>,
}

impl AppState {
    /// Creates a new application state.
    pub fn new(
        db_pool: PgPool,
        config: ServerConfig,
        version: String,
        prometheus_config: Option<crate::settings::PrometheusConfig>,
        grafana_cloud_prometheus_config: Option<crate::settings::GrafanaCloudPrometheusConfig>,
    ) -> Self {
        Self {
            db_pool,
            version,
            config: Arc::new(config),
            prometheus_config,
            grafana_cloud_prometheus_config,
        }
    }
}
