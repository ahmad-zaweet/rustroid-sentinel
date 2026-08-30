//! # Metrics Module
//!
//! Hybrid metrics collection: OTLP push to Grafana Cloud + Prometheus local scrape.
//!
//! This module provides dual-mode metrics collection:
//! 1. **OTLP Push**: Pushes metrics to Grafana Cloud at regular intervals
//! 2. **Prometheus Scrape**: Exposes a `/metrics` endpoint for local Prometheus scraping

pub mod error;
pub mod middleware;
pub mod otlp;
pub mod prometheus;
pub mod registry;
pub mod types;

// Re-export main items for backward compatibility
pub use middleware::{metrics_middleware, record_database_query};
pub use otlp::init_metrics;
pub use registry::{get_metrics, record_cache_result};
pub use types::MetricsSummary;

use crate::metrics::prometheus::{query_grafana_prometheus, query_local_prometheus};
use crate::settings::{GrafanaCloudPrometheusConfig, PrometheusConfig};

/// Gets a summary of key metrics from available sources.
///
/// # Priority
///
/// 1. Grafana Cloud Prometheus (if configured and valid)
/// 2. Legacy Prometheus config query URL
/// 3. Local Prometheus registry
/// 4. Database metrics only (if all else fails)
pub async fn get_metrics_summary(
    prometheus_config: &Option<PrometheusConfig>,
    grafana_cloud_config: &Option<GrafanaCloudPrometheusConfig>,
    pool: &sqlx::PgPool,
) -> axum::Json<MetricsSummary> {
    let db_metrics = get_database_metrics(pool).await;

    if let Some(cfg) = grafana_cloud_config.as_ref().filter(|c| c.is_valid()) {
        match query_grafana_prometheus(cfg, db_metrics.clone()).await {
            Ok(summary) => return axum::Json(summary),
            Err(e) => {
                tracing::error!(error = %e, "Failed to query Grafana Prometheus");
            }
        }
    }

    if let Some(cfg) = prometheus_config.as_ref().filter(|c| c.query_url.is_some()) {
        let legacy_config = GrafanaCloudPrometheusConfig {
            url: cfg
                .query_url
                .clone()
                .expect("query_url should be present due to filter"),
            instance_id: cfg.username.clone(),
            token: cfg.token.clone(),
        };
        if let Ok(summary) = query_grafana_prometheus(&legacy_config, db_metrics.clone()).await {
            return axum::Json(summary);
        }
    }

    axum::Json(query_local_prometheus(db_metrics))
}

/// Stub implementation when metrics feature is disabled.
#[cfg(not(feature = "metrics"))]
pub async fn get_metrics_summary(
    _prometheus_config: &Option<PrometheusConfig>,
    _grafana_cloud_config: &Option<GrafanaCloudPrometheusConfig>,
    pool: &sqlx::PgPool,
) -> axum::Json<MetricsSummary> {
    let db_metrics = get_database_metrics(pool).await;
    axum::Json(MetricsSummary {
        total_asteroids: db_metrics.total_asteroids,
        total_approaches: db_metrics.total_approaches,
        hazardous_count: db_metrics.hazardous_count,
        last_etl_run: db_metrics.last_etl_run,
        database_size_bytes: db_metrics.database_size_bytes,
        storage_used_percent: if types::DEFAULT_STORAGE_BUDGET_BYTES > 0 {
            (db_metrics.database_size_bytes as f64 / types::DEFAULT_STORAGE_BUDGET_BYTES as f64)
                * 100.0
        } else {
            0.0
        },
        ..Default::default()
    })
}

/// Fetches database metrics for inclusion in the summary.
async fn get_database_metrics(pool: &sqlx::PgPool) -> types::DatabaseMetrics {
    let mut metrics = types::DatabaseMetrics::default();

    if let Ok(count) = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM asteroids")
        .fetch_one(pool)
        .await
    {
        metrics.total_asteroids = count;
    }

    if let Ok(count) = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM approaches")
        .fetch_one(pool)
        .await
    {
        metrics.total_approaches = count;
    }

    if let Ok(count) = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM asteroids WHERE is_potentially_hazardous = TRUE",
    )
    .fetch_one(pool)
    .await
    {
        metrics.hazardous_count = count;
    }

    if let Ok(timestamp) = sqlx::query_scalar::<_, Option<chrono::DateTime<chrono::Utc>>>(
        "SELECT completed_at FROM etl_events WHERE status = 'success' ORDER BY completed_at DESC LIMIT 1",
    )
    .fetch_one(pool)
    .await
    {
        metrics.last_etl_run = timestamp.map(|t| t.timestamp());
    }

    if let Ok(size) = sqlx::query_scalar::<_, i64>("SELECT pg_database_size(current_database())")
        .fetch_one(pool)
        .await
    {
        metrics.database_size_bytes = size;
    }

    metrics
}
