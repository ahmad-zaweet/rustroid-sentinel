//! # OTLP Metrics Exporter
//!
//! This module handles OpenTelemetry Protocol (OTLP) export to Grafana Cloud.

use base64::{Engine, engine::general_purpose::STANDARD};
use opentelemetry::{KeyValue, global};
use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
};
use opentelemetry_semantic_conventions::resource::{
    DEPLOYMENT_ENVIRONMENT_NAME, HOST_NAME, SERVICE_NAME, SERVICE_NAMESPACE, SERVICE_VERSION,
};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{info, warn};

use crate::metrics::error::MetricsError;
use crate::settings::PrometheusConfig;

/// Initializes the hybrid metrics collection system.
///
/// Depending on the configuration and feature flags, this setup:
/// 1. Initializes a local Prometheus registry for scraping via `/metrics`.
/// 2. (Optional) Initializes the OTLP exporter to push metrics to a remote
///    collector (e.g., Grafana Cloud) every 10 seconds.
///
/// # Arguments
///
/// * `prometheus_config` - Optional configuration for the OTLP exporter.
/// * `version` - The application version to include in resource attributes.
/// * `env` - The deployment environment (e.g., "production", "staging").
/// * `service` - The name of the service for service identification.
///
/// # Errors
///
/// Returns a [`MetricsError`] if OTLP initialization fails.
pub fn init_metrics(
    prometheus_config: &Option<PrometheusConfig>,
    version: &str,
    env: &str,
    service: &str,
) -> Result<(), MetricsError> {
    info!("Initializing hybrid metrics (OTLP push + Prometheus scrape)");

    if let Some(cfg) = prometheus_config {
        if !cfg.url.is_empty() {
            init_otlp(&cfg.url, &cfg.username, &cfg.token, version, env, service)?;
        } else {
            info!("OTLP push not configured (url is empty), only Prometheus scrape available");
        }
    } else {
        warn!("Prometheus configuration missing, only local /metrics endpoint available");
    }

    Ok(())
}

/// Initializes the OTLP metrics exporter for pushing to Grafana Cloud.
///
/// # Arguments
///
/// * `endpoint` - The OTLP gateway URL
/// * `username` - The Grafana instance ID or username for Basic Auth
/// * `token` - The Grafana API token or password for Basic Auth
/// * `version` - The application version to include in resource attributes
/// * `env` - The deployment environment (e.g., "production", "staging")
/// * `service` - The service name for identification in Grafana
///
/// # Errors
///
/// Returns a [`MetricsError`] if the OTLP exporter fails to build.
pub fn init_otlp(
    endpoint: &str,
    username: &str,
    token: &str,
    version: &str,
    env: &str,
    service: &str,
) -> Result<(), MetricsError> {
    let endpoint = normalize_otlp_endpoint(endpoint);

    let mut headers = HashMap::new();
    let encoded = STANDARD.encode(format!("{}:{}", username, token));
    headers.insert("Authorization".into(), format!("Basic {}", encoded));

    let resource = Resource::builder()
        .with_attributes(vec![
            KeyValue::new(SERVICE_NAME, service.to_string()),
            KeyValue::new(SERVICE_NAMESPACE, "sentinel"),
            KeyValue::new(SERVICE_VERSION, version.to_string()),
            KeyValue::new(DEPLOYMENT_ENVIRONMENT_NAME, env.to_string()),
            KeyValue::new(
                HOST_NAME,
                gethostname::gethostname().to_string_lossy().to_string(),
            ),
        ])
        .build();

    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_endpoint(&endpoint)
        .with_protocol(opentelemetry_otlp::Protocol::HttpBinary)
        .with_headers(headers)
        .build()
        .map_err(|e| MetricsError::OtlpExporterBuild(e.to_string()))?;

    let provider = SdkMeterProvider::builder()
        .with_reader(
            PeriodicReader::builder(exporter)
                .with_interval(Duration::from_secs(10))
                .build(),
        )
        .with_resource(resource)
        .build();

    global::set_meter_provider(provider);
    info!(endpoint = %endpoint, "OTLP metrics provider initialized");

    Ok(())
}

/// Normalizes the OTLP endpoint URL.
fn normalize_otlp_endpoint(url: &str) -> String {
    let url = url.trim_end_matches('/');
    if url.ends_with("/otlp") {
        url.to_string()
    } else {
        format!("{}/otlp/v1/metrics", url)
    }
}

/// Stub implementation when metrics feature is disabled.
#[cfg(not(feature = "metrics"))]
pub fn init_metrics(
    _prometheus_config: &Option<crate::settings::PrometheusConfig>,
    _version: &str,
    _env: &str,
    _service: &str,
) -> Result<(), crate::metrics::error::MetricsError> {
    Ok(())
}
