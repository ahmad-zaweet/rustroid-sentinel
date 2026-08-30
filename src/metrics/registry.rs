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

/// In-memory cache hit/miss counter, labeled by cache name (`stats`,
/// `catalog_list`, etc.) and result (`hit`/`miss`).
pub(crate) static CACHE_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    let m = IntCounterVec::new(
        Opts::new(
            "dashboard_cache_requests_total",
            "Total number of in-memory dashboard cache lookups",
        ),
        &["cache", "result"],
    )
    .unwrap_or_else(|e| {
        error!(error = %e, "Failed to create dashboard_cache_requests_total metric");
        panic!(
            "Failed to create dashboard_cache_requests_total metric: {}",
            e
        );
    });
    register_metric(m, "dashboard_cache_requests_total").unwrap_or_else(|e| {
        error!(error = %e, "Failed to register dashboard_cache_requests_total metric");
        panic!(
            "Failed to register dashboard_cache_requests_total metric: {}",
            e
        );
    })
});

/// Records one cache lookup outcome for `name` (`hit` or `miss`), to both
/// the local Prometheus registry (`/metrics` scrape) and, via
/// [`crate::metrics::middleware::record_cache_result_otlp`], the OTLP push
/// path Grafana Cloud actually receives.
pub fn record_cache_result(name: &str, hit: bool) {
    CACHE_REQUESTS_TOTAL
        .with_label_values(&[name, if hit { "hit" } else { "miss" }])
        .inc();
    crate::metrics::middleware::record_cache_result_otlp(name, hit);
}

/// Pure hit-rate calculation: `hits / (hits + misses) * 100`, or `0.0` when
/// there have been no lookups yet. Split out from [`cache_hit_rate_percent`]
/// so the arithmetic is unit-testable without touching the global registry.
fn hit_rate_percent(hits: f64, misses: f64) -> f64 {
    let total = hits + misses;
    if total <= 0.0 {
        return 0.0;
    }
    (hits / total) * 100.0
}

/// Aggregate in-memory dashboard cache hit rate across every cache, as a
/// percentage. Reads straight from the local [`REGISTRY`] rather than
/// through an HTTP scrape, since it always runs in the same process as the
/// counters it's reading.
pub fn cache_hit_rate_percent() -> f64 {
    let mut hits = 0.0;
    let mut misses = 0.0;

    for family in REGISTRY.gather() {
        if family.name() != "dashboard_cache_requests_total" {
            continue;
        }
        for metric in family.get_metric() {
            let value = metric.get_counter().value();
            let is_hit = metric
                .get_label()
                .iter()
                .any(|l| l.name() == "result" && l.value() == "hit");
            if is_hit {
                hits += value;
            } else {
                misses += value;
            }
        }
    }

    hit_rate_percent(hits, misses)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_rate_percent_no_lookups_is_zero() {
        assert_eq!(hit_rate_percent(0.0, 0.0), 0.0);
    }

    #[test]
    fn hit_rate_percent_all_hits_is_100() {
        assert_eq!(hit_rate_percent(10.0, 0.0), 100.0);
    }

    #[test]
    fn hit_rate_percent_all_misses_is_zero() {
        assert_eq!(hit_rate_percent(0.0, 10.0), 0.0);
    }

    #[test]
    fn hit_rate_percent_computes_ratio() {
        assert!((hit_rate_percent(3.0, 1.0) - 75.0).abs() < 1e-9);
    }

    // `CACHE_REQUESTS_TOTAL` is a process-wide static shared by every test
    // in this binary, so each test below uses its own unique label value to
    // stay isolated regardless of test execution order or parallelism.

    #[test]
    fn record_cache_result_increments_labeled_counter() {
        let name = "registry_test_cache_increment";
        let before_hit = CACHE_REQUESTS_TOTAL.with_label_values(&[name, "hit"]).get();
        let before_miss = CACHE_REQUESTS_TOTAL
            .with_label_values(&[name, "miss"])
            .get();

        record_cache_result(name, true);
        record_cache_result(name, true);
        record_cache_result(name, false);

        assert_eq!(
            CACHE_REQUESTS_TOTAL.with_label_values(&[name, "hit"]).get(),
            before_hit + 2
        );
        assert_eq!(
            CACHE_REQUESTS_TOTAL
                .with_label_values(&[name, "miss"])
                .get(),
            before_miss + 1
        );
    }

    #[test]
    fn cache_hit_rate_percent_reflects_recorded_results() {
        let name = "registry_test_cache_hit_rate";
        record_cache_result(name, true);
        record_cache_result(name, true);
        record_cache_result(name, true);
        record_cache_result(name, false);

        // Aggregate across all caches, so this can only assert a lower
        // bound: the 3 hits recorded here guarantee at least this many
        // "hit" observations exist process-wide.
        assert!(cache_hit_rate_percent() > 0.0);
    }
}
