//! # Prometheus Query Functions
//!
//! This module provides functions for querying Prometheus and Grafana Cloud APIs
//! to retrieve metrics data.

use reqwest;
use serde::Deserialize;
use tracing::debug;

use crate::metrics::error::MetricsError;
use crate::metrics::registry::cache_hit_rate_percent;
use crate::metrics::types::{DEFAULT_STORAGE_BUDGET_BYTES, DatabaseMetrics, MetricsSummary};
use crate::settings::GrafanaCloudPrometheusConfig;

fn storage_used_percent(database_size_bytes: i64, storage_budget_bytes: i64) -> f64 {
    if storage_budget_bytes <= 0 {
        return 0.0;
    }
    (database_size_bytes as f64 / storage_budget_bytes as f64) * 100.0
}

/// Queries Grafana Cloud Prometheus for metrics summary.
pub(crate) async fn query_grafana_prometheus(
    config: &GrafanaCloudPrometheusConfig,
    db_metrics: DatabaseMetrics,
) -> Result<MetricsSummary, MetricsError> {
    let client = reqwest::Client::new();
    let base_url = config.url.trim_end_matches('/');

    // `[30d]` rather than a short rolling window: PromQL's `rate()` averages
    // over whatever actual samples fall inside the window, so a range this
    // wide effectively means "since the process started" for any realistic
    // uptime, instead of a per-minute snapshot that swings to 0 the moment
    // traffic goes quiet.
    let queries = PrometheusQueries {
        requests_per_second: "sum(rate(http_requests_total[30d]))",
        error_rate_percent: "sum(rate(http_requests_total{status=~\"4..|5..\"}[30d])) / sum(rate(http_requests_total[30d])) * 100",
        avg_response_time_ms: "sum(rate(http_request_duration_seconds_sum[30d])) / sum(rate(http_request_duration_seconds_count[30d])) * 1000",
        db_queries_per_second: "sum(rate(database_queries_total[30d]))",
        cache_hit_rate_percent: "sum(rate(dashboard_cache_requests_total{result=\"hit\"}[30d])) / sum(rate(dashboard_cache_requests_total[30d])) * 100",
    };

    let mut summary = MetricsSummary {
        total_asteroids: db_metrics.total_asteroids,
        total_approaches: db_metrics.total_approaches,
        hazardous_count: db_metrics.hazardous_count,
        last_etl_run: db_metrics.last_etl_run,
        database_size_bytes: db_metrics.database_size_bytes,
        storage_used_percent: storage_used_percent(
            db_metrics.database_size_bytes,
            DEFAULT_STORAGE_BUDGET_BYTES,
        ),
        ..Default::default()
    };

    if let Ok(val) =
        fetch_prometheus_value(&client, base_url, config, queries.requests_per_second).await
    {
        summary.requests_per_second = val;
    }

    if let Ok(val) =
        fetch_prometheus_value(&client, base_url, config, queries.error_rate_percent).await
    {
        summary.error_rate_percent = val;
    }

    if let Ok(val) =
        fetch_prometheus_value(&client, base_url, config, queries.avg_response_time_ms).await
    {
        summary.avg_response_time_ms = val;
    }

    if let Ok(val) =
        fetch_prometheus_value(&client, base_url, config, queries.db_queries_per_second).await
    {
        summary.db_queries_per_second = val;
    }

    if let Ok(val) =
        fetch_prometheus_value(&client, base_url, config, queries.cache_hit_rate_percent).await
    {
        summary.cache_hit_rate_percent = val;
    }

    Ok(summary)
}

/// Prometheus query definitions.
struct PrometheusQueries<'a> {
    requests_per_second: &'a str,
    error_rate_percent: &'a str,
    avg_response_time_ms: &'a str,
    db_queries_per_second: &'a str,
    cache_hit_rate_percent: &'a str,
}

/// Fetches a single metric value from Prometheus.
async fn fetch_prometheus_value(
    client: &reqwest::Client,
    base_url: &str,
    config: &GrafanaCloudPrometheusConfig,
    query: &str,
) -> Result<f64, MetricsError> {
    let url = format!("{}?query={}", base_url, urlencoding::encode(query));

    debug!("Querying Prometheus: {}", url);

    let mut req = client.get(&url);
    req = req.basic_auth(config.username(), Some(config.password()));

    let resp = req.send().await.map_err(MetricsError::from)?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_body = resp.text().await.unwrap_or_else(|_| "N/A".to_string());
        return Err(MetricsError::PrometheusQuery(format!(
            "Prometheus query failed ({}): {}",
            status, error_body
        )));
    }

    let json: PrometheusResponse = resp.json().await.map_err(MetricsError::from)?;

    let result = json
        .data
        .result
        .first()
        .and_then(|r| r.value.get(1))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    Ok(result)
}

/// Prometheus API response structure.
#[derive(Debug, Deserialize)]
struct PrometheusResponse {
    #[allow(dead_code)]
    status: String,
    data: PrometheusData,
}

#[derive(Debug, Deserialize)]
struct PrometheusData {
    result: Vec<PrometheusResult>,
}

#[derive(Debug, Deserialize)]
struct PrometheusResult {
    value: Vec<serde_json::Value>,
}

/// Queries local Prometheus registry for metrics.
pub(crate) fn query_local_prometheus(db_metrics: DatabaseMetrics) -> MetricsSummary {
    MetricsSummary {
        requests_per_second: 0.0,
        error_rate_percent: 0.0,
        avg_response_time_ms: 0.0,
        db_queries_per_second: 0.0,
        total_asteroids: db_metrics.total_asteroids,
        total_approaches: db_metrics.total_approaches,
        hazardous_count: db_metrics.hazardous_count,
        last_etl_run: db_metrics.last_etl_run,
        database_size_bytes: db_metrics.database_size_bytes,
        storage_budget_bytes: DEFAULT_STORAGE_BUDGET_BYTES,
        storage_used_percent: storage_used_percent(
            db_metrics.database_size_bytes,
            DEFAULT_STORAGE_BUDGET_BYTES,
        ),
        cache_hit_rate_percent: cache_hit_rate_percent(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_used_percent_computes_ratio() {
        assert!((storage_used_percent(256 * 1024 * 1024, 512 * 1024 * 1024) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn storage_used_percent_zero_budget_is_zero() {
        assert_eq!(storage_used_percent(100, 0), 0.0);
    }

    #[test]
    fn storage_used_percent_zero_usage_is_zero() {
        assert_eq!(storage_used_percent(0, 512 * 1024 * 1024), 0.0);
    }

    #[test]
    fn query_local_prometheus_carries_storage_fields() {
        let db_metrics = DatabaseMetrics {
            total_asteroids: 10,
            total_approaches: 20,
            hazardous_count: 3,
            last_etl_run: None,
            database_size_bytes: 400 * 1024 * 1024,
        };

        let summary = query_local_prometheus(db_metrics);

        assert_eq!(summary.database_size_bytes, 400 * 1024 * 1024);
        assert_eq!(summary.storage_budget_bytes, DEFAULT_STORAGE_BUDGET_BYTES);
        assert!(summary.storage_used_percent > 0.0);
    }
}
