//! # Metrics Error Types
//!
//! This module defines error types for metrics collection and reporting operations.
//! It provides structured errors for metric registration, OTLP export, and Prometheus queries.

use thiserror::Error;

/// Errors that can occur during metrics operations.
#[derive(Error, Debug)]
pub enum MetricsError {
    /// Failed to register a metric with the Prometheus registry.
    #[error("failed to register metric '{metric_name}': {source}")]
    MetricRegistration {
        /// The name of the metric that failed to register.
        metric_name: String,
        /// The underlying registration error.
        #[source]
        source: prometheus::Error,
    },

    /// Failed to create a metric (histogram, counter, etc.).
    #[error("failed to create metric '{metric_name}': {source}")]
    MetricCreation {
        /// The name of the metric that failed to create.
        metric_name: String,
        /// The underlying creation error.
        #[source]
        source: prometheus::Error,
    },

    /// Failed to build the OTLP exporter.
    #[error("failed to build OTLP exporter: {0}")]
    OtlpExporterBuild(String),

    /// Failed to initialize the OTLP metrics provider.
    #[error("failed to initialize OTLP provider: {0}")]
    OtlpProviderInit(String),

    /// Failed to encode Prometheus metrics.
    #[error("failed to encode Prometheus metrics: {0}")]
    MetricEncoding(#[from] prometheus::Error),

    /// Failed to query Prometheus/Grafana API.
    #[error("Prometheus query failed: {0}")]
    PrometheusQuery(String),

    /// Failed to parse Prometheus response.
    #[error("failed to parse Prometheus response: {0}")]
    PrometheusParse(String),

    /// HTTP request to metrics backend failed.
    #[error("HTTP request to metrics backend failed: {0}")]
    HttpRequest(#[from] reqwest::Error),

    /// Invalid metrics endpoint URL.
    #[error("invalid metrics endpoint URL: {0}")]
    InvalidEndpoint(String),

    /// Metrics system not initialized.
    #[error("metrics system not initialized")]
    NotInitialized,
}

impl MetricsError {
    /// Returns true if the error is retryable.
    ///
    /// Network errors and temporary service unavailability are considered retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::HttpRequest(e) => e.is_timeout() || e.is_connect(),
            Self::PrometheusQuery(_) | Self::OtlpProviderInit(_) => true,
            _ => false,
        }
    }

    /// Returns true if the error is a configuration error.
    pub fn is_config_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidEndpoint(_)
                | Self::OtlpExporterBuild(_)
                | Self::MetricCreation { .. }
                | Self::MetricRegistration { .. }
        )
    }
}

/// Result type alias for metrics operations.
pub type Result<T> = std::result::Result<T, MetricsError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_error_display() {
        let err = MetricsError::NotInitialized;
        assert_eq!(format!("{}", err), "metrics system not initialized");

        let err = MetricsError::InvalidEndpoint("invalid-url".to_string());
        assert_eq!(
            format!("{}", err),
            "invalid metrics endpoint URL: invalid-url"
        );
    }

    #[test]
    fn test_is_retryable() {
        let retryable = MetricsError::PrometheusQuery("timeout".to_string());
        assert!(retryable.is_retryable());

        let non_retryable = MetricsError::NotInitialized;
        assert!(!non_retryable.is_retryable());
    }

    #[test]
    fn test_is_config_error() {
        let config_err = MetricsError::InvalidEndpoint("bad".to_string());
        assert!(config_err.is_config_error());

        // Test with a different error variant since reqwest::Error can't be default constructed
        let network_err = MetricsError::PrometheusQuery("network error".to_string());
        assert!(!network_err.is_config_error());
    }
}
