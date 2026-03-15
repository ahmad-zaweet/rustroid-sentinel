//! # Prometheus Metrics Registry
//!
//! This module manages the Prometheus metrics registry and metric definitions.
//! It provides thread-safe access to metrics counters and histograms.

use axum::response::IntoResponse;
use once_cell::sync::Lazy;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry, TextEncoder,
    core::Collector,
};
use tracing::{error, info};

use crate::metrics::error::MetricsError;

/// Global Prometheus registry instance.
pub(crate) static REGISTRY: Lazy<Registry> = Lazy::new(|| {
    let registry = Registry::new();
    info!("Initialized Prometheus metrics registry");
    registry
});

/// Helper function to register a metric with error logging.
pub(crate) fn register_metric<M: Collector + Clone + 'static>(
    metric: M,
    name: &str,
) -> Result<M, MetricsError> {
    REGISTRY
        .register(Box::new(metric.clone()))
        .map_err(|e| MetricsError::MetricRegistration {
            metric_name: name.to_string(),
            source: e,
        })?;
    Ok(metric)
}

/// HTTP requests total counter.
pub(crate) static HTTP_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let m = IntCounterVec::new(
        Opts::new("http_requests_total", "Total number of HTTP requests"),
        &["method", "path", "status"],
    )
    .unwrap_or_else(|e| {
        error!(error = %e, "Failed to create http_requests_total metric");
        panic!("Failed to create http_requests_total metric: {}", e);
    });
    register_metric(m, "http_requests_total").unwrap_or_else(|e| {
        error!(error = %e, "Failed to register http_requests_total metric");
        panic!("Failed to register http_requests_total metric: {}", e);
    })
});

/// HTTP request duration histogram.
pub(crate) static HTTP_REQUEST_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    let m = HistogramVec::new(
        HistogramOpts::new(
            "http_request_duration_seconds",
            "HTTP request duration in seconds",
        )
        .buckets(vec![
            0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ]),
        &["method", "path", "status"],
    )
    .unwrap_or_else(|e| {
        error!(error = %e, "Failed to create http_request_duration_seconds metric");
        panic!(
            "Failed to create http_request_duration_seconds metric: {}",
            e
        );
    });
    register_metric(m, "http_request_duration_seconds").unwrap_or_else(|e| {
        error!(error = %e, "Failed to register http_request_duration_seconds metric");
        panic!(
            "Failed to register http_request_duration_seconds metric: {}",
            e
        );
    })
});

/// Database queries total counter.
pub(crate) static DATABASE_QUERIES_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let m = IntCounterVec::new(
        Opts::new("database_queries_total", "Total number of database queries"),
        &["operation", "status"],
    )
    .unwrap_or_else(|e| {
        error!(error = %e, "Failed to create database_queries_total metric");
        panic!("Failed to create database_queries_total metric: {}", e);
    });
    register_metric(m, "database_queries_total").unwrap_or_else(|e| {
        error!(error = %e, "Failed to register database_queries_total metric");
        panic!("Failed to register database_queries_total metric: {}", e);
    })
});

/// Database query duration histogram.
pub(crate) static DATABASE_QUERY_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    let m = HistogramVec::new(
        HistogramOpts::new(
            "database_query_duration_seconds",
            "Database query duration in seconds",
        )
        .buckets(vec![
            0.0001, 0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
        ]),
        &["operation", "status"],
    )
    .unwrap_or_else(|e| {
        error!(error = %e, "Failed to create database_query_duration_seconds metric");
        panic!(
            "Failed to create database_query_duration_seconds metric: {}",
            e
        );
    });
    register_metric(m, "database_query_duration_seconds").unwrap_or_else(|e| {
        error!(error = %e, "Failed to register database_query_duration_seconds metric");
        panic!(
            "Failed to register database_query_duration_seconds metric: {}",
            e
        );
    })
});

/// Returns Prometheus-formatted metrics for scraping.
///
/// # Returns
///
/// - `200 OK` with Prometheus metrics body on success
/// - `500 Internal Server Error` if encoding fails
pub async fn get_metrics() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metrics = REGISTRY.gather();

    let mut buffer = Vec::new();
    if let Err(e) = encoder.encode(&metrics, &mut buffer) {
        error!(error = %e, "Failed to encode Prometheus metrics");
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to encode metrics",
        )
            .into_response();
    }

    (axum::http::StatusCode::OK, axum::body::Body::from(buffer)).into_response()
}

/// Stub handler for when metrics feature is disabled.
#[cfg(not(feature = "metrics"))]
pub async fn get_metrics() -> impl IntoResponse {
    (axum::http::StatusCode::NOT_IMPLEMENTED, "Metrics disabled")
}
