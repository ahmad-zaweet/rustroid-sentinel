//! # HTTP Server Module
//!
//! This module provides the Axum-based HTTP server for the Rustroid Sentinel API.

pub mod middleware;
pub mod router;
pub mod shutdown;
pub mod state;

pub use shutdown::shutdown_signal;
pub use state::AppState;

use lazy_limit::{Duration as LazyDuration, RuleConfig, init_rate_limiter};
use sqlx::PgPool;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

use crate::settings::{DatabaseConfig, ServerConfig, ServiceConfig};

#[allow(clippy::cognitive_complexity)]
/// Configures and starts the Axum HTTP server.
pub async fn run_server(
    service_config: &ServiceConfig,
    server_config: &ServerConfig,
    database_config: &DatabaseConfig,
    prometheus_config: Option<crate::settings::PrometheusConfig>,
    grafana_cloud_prometheus_config: Option<crate::settings::GrafanaCloudPrometheusConfig>,
    db_pool: PgPool,
    version: String,
) -> Result<(), std::io::Error> {
    // Initialize Prometheus metrics
    crate::metrics::init_metrics(
        &prometheus_config,
        &version,
        &service_config.env,
        &service_config.name,
    )
    .map_err(|e| std::io::Error::other(format!("Metrics initialization failed: {}", e)))?;

    let addr: SocketAddr = format!("{}:{}", service_config.host, service_config.port)
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let timeout = std::time::Duration::from_secs(server_config.request_timeout_seconds);

    // Initialize rate limiting rules (global)
    init_rate_limiter!(
        default: RuleConfig::new(
            LazyDuration::seconds(server_config.rate_limit_period_seconds),
            server_config.rate_limit_requests as u32,
        )
    )
    .await;

    let internal_event_token: Arc<str> = std::env::var("INTERNAL_EVENT_TOKEN")
        .map_err(|_| {
            std::io::Error::other(
                "INTERNAL_EVENT_TOKEN env var must be set (required to authenticate POST /internal/events)",
            )
        })?
        .into();

    let events_tx = crate::events::channel();

    #[cfg(feature = "pg-listen")]
    {
        if let Some(listen_url) = database_config.listen_url.clone() {
            let events_tx = events_tx.clone();
            tokio::spawn(async move {
                if let Err(error) = crate::events::pg_listen::run(&listen_url, events_tx).await {
                    tracing::error!(%error, "PgListenSource terminated");
                }
            });
        } else {
            return Err(std::io::Error::other(
                "feature \"pg-listen\" is enabled but database.listen_url is unset",
            ));
        }
    }
    #[cfg(not(feature = "pg-listen"))]
    let _ = database_config;

    let state = AppState::new(
        db_pool,
        server_config.clone(),
        version.clone(),
        prometheus_config,
        grafana_cloud_prometheus_config,
        events_tx,
        internal_event_token,
    );

    info!(
        name = %service_config.name,
        version = %version,
        addr = %addr,
        env = %service_config.env,
        timeout = server_config.request_timeout_seconds,
        rate_limit = server_config.rate_limit_requests,
        "Starting HTTP server"
    );

    let listener = TcpListener::bind(addr).await?;
    let app = router::build_router(state, timeout, server_config.clone());

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    info!("Server shutdown complete");
    Ok(())
}
