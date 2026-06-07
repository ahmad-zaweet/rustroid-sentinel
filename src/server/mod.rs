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
use tokio::net::TcpListener;
use tracing::info;

use crate::settings::{ServerConfig, ServiceConfig};

#[allow(clippy::cognitive_complexity)]
/// Configures and starts the Axum HTTP server.
pub async fn run_server(
    service_config: &ServiceConfig,
    server_config: &ServerConfig,
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

    let state = AppState::new(
        db_pool,
        server_config.clone(),
        version.clone(),
        prometheus_config,
        grafana_cloud_prometheus_config,
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

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server shutdown complete");
    Ok(())
}
