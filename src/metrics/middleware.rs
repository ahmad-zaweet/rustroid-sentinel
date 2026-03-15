//! # Metrics Middleware
//!
//! This module provides Axum middleware for automatic request timing and metrics recording.

use axum::{extract::Request, middleware::Next, response::Response};
use opentelemetry::KeyValue;
use std::time::Instant;

use crate::metrics::registry::{
    DATABASE_QUERIES_TOTAL, DATABASE_QUERY_DURATION, HTTP_REQUEST_DURATION, HTTP_REQUESTS_TOTAL,
};

/// A timer for measuring HTTP request duration and recording metrics.
#[derive(Debug)]
pub struct MetricsTimer {
    start: Instant,
    method: String,
    path: String,
}

impl MetricsTimer {
    /// Creates a new timer for measuring request duration.
    pub fn new(method: &str, path: &str) -> Self {
        Self {
            start: Instant::now(),
            method: method.to_string(),
            path: path.to_string(),
        }
    }

    /// Records the metrics when the request completes.
    pub fn finish(self, status: u16) {
        let duration = self.start.elapsed().as_secs_f64();
        let status_str = status.to_string();
        let labels = [
            self.method.as_str(),
            self.path.as_str(),
            status_str.as_str(),
        ];

        HTTP_REQUESTS_TOTAL.with_label_values(&labels).inc();
        HTTP_REQUEST_DURATION
            .with_label_values(&labels)
            .observe(duration);

        // Record OTLP metrics
        let otlp_labels = [
            KeyValue::new("http.request.method", self.method.clone()),
            KeyValue::new("url.path", self.path.clone()),
            KeyValue::new("http.response.status_code", status as i64),
        ];
        OTLP_HTTP_REQUESTS.add(1, &otlp_labels);
        OTLP_HTTP_DURATION.record(duration, &otlp_labels);
    }
}

/// Axum middleware for automatically recording HTTP request metrics.
pub async fn metrics_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone().to_string();
    let path = request.uri().path().to_string();

    let timer = MetricsTimer::new(&method, &path);
    let response = next.run(request).await;
    let status = response.status().as_u16();

    timer.finish(status);
    response
}

/// Stub middleware for when metrics feature is disabled.
#[cfg(not(feature = "metrics"))]
pub async fn metrics_middleware(request: Request, next: Next) -> axum::response::Response {
    next.run(request).await
}

// OTLP metrics (separate from Prometheus)
static OTLP_HTTP_REQUESTS: once_cell::sync::Lazy<opentelemetry::metrics::Counter<u64>> =
    once_cell::sync::Lazy::new(|| {
        opentelemetry::global::meter("rustroid-sentinel")
            .u64_counter("http_requests_total")
            .with_description("Total number of HTTP requests")
            .build()
    });

static OTLP_HTTP_DURATION: once_cell::sync::Lazy<opentelemetry::metrics::Histogram<f64>> =
    once_cell::sync::Lazy::new(|| {
        opentelemetry::global::meter("rustroid-sentinel")
            .f64_histogram("http_request_duration_seconds")
            .with_description("HTTP request duration in seconds")
            .build()
    });

static OTLP_DB_QUERIES: once_cell::sync::Lazy<opentelemetry::metrics::Counter<u64>> =
    once_cell::sync::Lazy::new(|| {
        opentelemetry::global::meter("rustroid-sentinel")
            .u64_counter("database_queries_total")
            .with_description("Total number of database queries")
            .build()
    });

/// Records a database query metric.
///
/// # Arguments
///
/// * `operation` - The type of operation (e.g., "SELECT", "INSERT")
/// * `duration_secs` - The query duration in seconds
/// * `success` - Whether the query completed successfully
pub fn record_database_query(operation: &str, duration_secs: f64, success: bool) {
    let status = if success { "success" } else { "error" };
    let labels = [operation, status];

    DATABASE_QUERIES_TOTAL.with_label_values(&labels).inc();
    DATABASE_QUERY_DURATION
        .with_label_values(&labels)
        .observe(duration_secs);

    // Record OTLP metrics
    let otlp_labels = [
        KeyValue::new("db.system", "postgresql"),
        KeyValue::new("db.operation", operation.to_string()),
        KeyValue::new("db.response.status", status.to_string()),
    ];
    OTLP_DB_QUERIES.add(1, &otlp_labels);
}

/// Stub function when metrics feature is disabled.
#[cfg(not(feature = "metrics"))]
pub fn record_database_query(_operation: &str, _duration_secs: f64, _success: bool) {}
