//! # Prometheus Query Functions
//!
//! This module provides functions for querying Prometheus and Grafana Cloud APIs
//! to retrieve metrics data.

use reqwest;
use serde::Deserialize;
use tracing::debug;

use crate::metrics::error::MetricsError;
use crate::metrics::types::{DatabaseMetrics, MetricsSummary};
use crate::settings::GrafanaCloudPrometheusConfig;

/// Queries Grafana Cloud Prometheus for metrics summary.
pub(crate) async fn query_grafana_prometheus(
    config: &GrafanaCloudPrometheusConfig,
    db_metrics: DatabaseMetrics,
) -> Result<MetricsSummary, MetricsError> {
    let client = reqwest::Client::new();
    let base_url = config.url.trim_end_matches('/');

    let queries = PrometheusQueries {
        requests_per_second: "sum(rate(http_requests_total[1m]))",
        error_rate_percent: "sum(rate(http_requests_total{status=~\"4..|5..\"}[1m])) / sum(rate(http_requests_total[1m])) * 100",
        avg_response_time_ms: "sum(rate(http_request_duration_seconds_sum[1m])) / sum(rate(http_request_duration_seconds_count[1m])) * 1000",
        db_queries_per_second: "sum(rate(database_queries_total[1m]))",
    };

    let mut summary = MetricsSummary {
        total_asteroids: db_metrics.total_asteroids,
        total_approaches: db_metrics.total_approaches,
        hazardous_count: db_metrics.hazardous_count,
        last_etl_run: db_metrics.last_etl_run,
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

    Ok(summary)
}

/// Prometheus query definitions.
struct PrometheusQueries<'a> {
    requests_per_second: &'a str,
    error_rate_percent: &'a str,
    avg_response_time_ms: &'a str,
    db_queries_per_second: &'a str,
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
    }
}
