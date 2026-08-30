//! # Configuration Management
//!
//! This module provides configuration management for the Rustroid Sentinel application.
//! Configuration is loaded from multiple sources with later sources overriding earlier ones:
//!
//! 1. **Base Configuration**: `config/config.toml` (optional)
//! 2. **Environment-Specific**: `config/{RUN_ENV}.toml` (required, defaults to `development.toml`)
//! 3. **Environment Variables**: Prefixed with `SERVICE__` (e.g., `SERVICE__DATABASE__URL`)
//!
//! ## Configuration Structure
//!
//! | Section | Description | Key Fields |
//! |---------|-------------|------------|
//! | `service` | General service settings | `name`, `host`, `port`, `log_level` |
//! | `database` | PostgreSQL connection | `url`, `max_connections`, `timeout` |
//! | `nasa` | NASA API client | `api_key`, `base_url`, `max_retries` |
//! | `discord` | Discord webhook | `webhook_url`, `timeout` |
//! | `http` | HTTP client settings | `user_agent`, `timeout`, `gzip` |
//! | `etl` | ETL process settings | `fetch_interval_hours`, `batch_size` |
//! | `server` | HTTP server settings | `rate_limit` |
//! | `prometheus` | Metrics configuration | `url`, `query_url`, `interval` |
//! | `jpl_sentry` | JPL Sentry impact-monitoring client | `base_url`, `request_delay_ms`, `stale_days` |
//! | `jpl_sbdb` | JPL Small-Body Database (orbital elements) client | `base_url`, `request_delay_ms`, `stale_days` |
//!
//! ## Example Configuration
//!
//! ```toml
//! [service]
//! name = "rustroid-sentinel"
//! host = "0.0.0.0"
//! port = 8080
//! log_level = "info"
//!
//! [database]
//! url = "postgresql://postgres:password@localhost:5432/rustroid_sentinel"
//! max_connections = 10
//!
//! [nasa]
//! api_key = "DEMO_KEY"
//! base_url = "https://api.nasa.gov"
//! ```
//!
//! ## Environment Variable Override
//!
//! ```bash
//! export SERVICE__DATABASE__URL="postgresql://prod:secret@db.example.com/prod_db"
//! export SERVICE__NASA__API_KEY="production_key"
//! ```

use crate::error::ServiceConfigError;
use config::{Config, Environment, File};
use serde::Deserialize;
use std::env;

/// The root configuration struct for the entire application.
///
/// This struct aggregates all configuration modules into a single accessible point.
#[derive(Debug, Deserialize, Clone)]
pub struct RustroidSentinelConfig {
    /// Service-specific settings, such as port, host, and logging.
    pub service: ServiceConfig,
    /// Configuration for the database connection pool.
    pub database: DatabaseConfig,
    /// Configuration for the NASA API client.
    pub nasa: NasaConfig,
    /// Configuration for Discord webhook notifications.
    pub discord: DiscordConfig,
    /// Generic HTTP client settings.
    pub http: HttpConfig,
    /// Settings for the ETL (Extract, Transform, Load) process.
    pub etl: EtlConfig,
    /// Configuration for the HTTP server (Axum).
    pub server: ServerConfig,
    /// Configuration for Prometheus metrics push.
    pub prometheus: Option<PrometheusConfig>,
    /// Configuration for Grafana Cloud Prometheus querying (dashboard metrics).
    pub grafana_cloud_prometheus: Option<GrafanaCloudPrometheusConfig>,
    /// Configuration for the JPL Sentry impact-monitoring API client.
    #[serde(default)]
    pub jpl_sentry: JplSentryConfig,
    /// Configuration for the JPL Small-Body Database (orbital elements) client.
    #[serde(default)]
    pub jpl_sbdb: JplSbdbConfig,
}

fn default_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Contains general service settings.
#[derive(Debug, Deserialize, Clone)]
pub struct ServiceConfig {
    /// The name of the service, used for identification in logs and metrics.
    pub name: String,
    /// The runtime environment (e.g., "development", "production").
    pub env: String,
    /// The IP address to which the service will bind.
    pub host: String,
    /// The port on which the service will listen for incoming connections.
    pub port: u16,
    /// The logging level for the service (e.g., "info", "debug", "warn").
    pub log_level: String,
    /// The version set for the service.
    #[serde(default = "default_version")]
    pub version: String,
}

/// Holds all settings related to the PostgreSQL database connection.
#[derive(Deserialize, Clone)]
pub struct DatabaseConfig {
    /// The full database connection URL.
    /// Example: "postgresql://user:password@localhost:5432/mydatabase"
    pub url: String,
    /// Direct (non-pooled) connection URL, required for `LISTEN`/`NOTIFY`.
    ///
    /// Neon's default pooled endpoint (`-pooler`) is PgBouncer in transaction
    /// mode, which does not support `LISTEN`. When set, this bypasses the
    /// pooler for the dedicated listener connection only; `url` above keeps
    /// serving the normal pool. Not set on non-serverless/dedicated Postgres
    /// deployments, where `url` already points at a direct connection.
    #[serde(default)]
    pub listen_url: Option<String>,
    /// The maximum number of connections the pool can hold.
    pub max_connections: u32,
    /// The minimum number of idle connections to maintain in the pool.
    pub min_connections: u32,
    /// The timeout in seconds for establishing a new database connection.
    pub connect_timeout_seconds: u32,
}

impl std::fmt::Debug for DatabaseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseConfig")
            .field("url", &"[REDACTED]")
            .field(
                "listen_url",
                &self.listen_url.as_ref().map(|_| "[REDACTED]"),
            )
            .field("max_connections", &self.max_connections)
            .field("min_connections", &self.min_connections)
            .field("connect_timeout_seconds", &self.connect_timeout_seconds)
            .finish()
    }
}

/// Configuration for the client that interacts with NASA APIs.
#[derive(Deserialize, Clone)]
pub struct NasaConfig {
    /// The API key required for authenticating with NASA APIs.
    pub api_key: String,
    /// The base URL for all NASA API endpoints.
    pub base_url: String,
    /// The timeout in seconds for requests made to the NASA API.
    pub timeout_seconds: u64,
    /// The maximum number of times to retry a failed request.
    pub max_retries: u32,
    /// The delay in milliseconds between request retries.
    pub retry_delay_ms: u64,
    /// The maximum number of concurrent requests to the NASA API.
    #[serde(default = "default_max_concurrent_requests")]
    pub max_concurrent_requests: usize,
}

fn default_max_concurrent_requests() -> usize {
    5
}

impl std::fmt::Debug for NasaConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NasaConfig")
            .field("api_key", &"[REDACTED]")
            .field("base_url", &self.base_url)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("max_retries", &self.max_retries)
            .field("retry_delay_ms", &self.retry_delay_ms)
            .field("max_concurrent_requests", &self.max_concurrent_requests)
            .finish()
    }
}

/// Configuration for the client that interacts with JPL's Sentry impact-monitoring
/// API (`https://ssd-api.jpl.nasa.gov/sentry.api`). Unlike NeoWs, this API is
/// public and needs no key; `request_delay_ms` is a self-imposed courtesy pause
/// between lookups, since the API exposes no rate-limit headers to react to.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct JplSentryConfig {
    /// The base URL for the Sentry API.
    pub base_url: String,
    /// Delay in milliseconds between successive per-asteroid lookups.
    pub request_delay_ms: u32,
    /// How many days a `sentry_checked_at` stamp is considered fresh before
    /// the `sentry` CLI command re-checks that asteroid.
    pub stale_days: u32,
}

impl Default for JplSentryConfig {
    fn default() -> Self {
        Self {
            base_url: "https://ssd-api.jpl.nasa.gov/sentry.api".to_string(),
            request_delay_ms: 1000,
            stale_days: 30,
        }
    }
}

/// Configuration for the client that interacts with JPL's Small-Body Database
/// API (`https://ssd-api.jpl.nasa.gov/sbdb.api`), used to fetch orbital
/// elements. Like Sentry, it's public and needs no key; `request_delay_ms` is
/// a self-imposed courtesy pause between lookups.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct JplSbdbConfig {
    /// The base URL for the SBDB API.
    pub base_url: String,
    /// Delay in milliseconds between successive per-asteroid lookups.
    pub request_delay_ms: u64,
    /// How many days an `orbit_checked_at` stamp is considered fresh before
    /// the `orbits` CLI command re-checks that asteroid.
    pub stale_days: u32,
}

impl Default for JplSbdbConfig {
    fn default() -> Self {
        Self {
            base_url: "https://ssd-api.jpl.nasa.gov/sbdb.api".to_string(),
            request_delay_ms: 1000,
            stale_days: 90,
        }
    }
}

/// Configuration for sending notifications via a Discord webhook.
#[derive(Deserialize, Clone)]
pub struct DiscordConfig {
    /// The URL of the Discord webhook to which notifications will be sent.
    pub webhook_url: String,
    /// The timeout in seconds for the request to send a notification.
    pub timeout_seconds: u32,
    /// The maximum number of retries for a failed notification request.
    pub max_retries: u32,
}

impl std::fmt::Debug for DiscordConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscordConfig")
            .field("webhook_url", &"[REDACTED]")
            .field("timeout_seconds", &self.timeout_seconds)
            .field("max_retries", &self.max_retries)
            .finish()
    }
}

fn default_enable_gzip() -> bool {
    true
}

/// Generic configuration for HTTP clients used throughout the application.
#[derive(Debug, Deserialize, Clone)]
pub struct HttpConfig {
    /// The User-Agent string to be included in all outgoing HTTP requests.
    pub user_agent: String,
    /// The overall request timeout in seconds.
    pub timeout_seconds: u64,
    /// The timeout in seconds for the connection phase of a request.
    pub connect_timeout_seconds: u64,
    /// The time in seconds a connection can remain idle in the pool before being closed.
    pub pool_idle_timeout_seconds: u64,
    /// The maximum number of idle connections to maintain per host.
    pub pool_max_idle_per_host: usize,
    /// A boolean flag to indicate whether to enable gzip compression for requests or not.
    #[serde(default = "default_enable_gzip")]
    pub enable_gzip: bool,
}

/// Configuration for the ETL process responsible for fetching and processing data.
#[derive(Debug, Deserialize, Clone)]
pub struct EtlConfig {
    /// The interval in hours at which the data fetching process should run.
    pub fetch_interval_hours: u32,
    /// The number of past days of data to fetch during each run.
    pub lookback_days: u32,
    /// The number of future days of data to fetch during each run.
    pub lookahead_days: u32,
    /// The cooldown period in hours before sending a new alert for the same event.
    pub alert_cooldown_hours: u32,
    /// The number of records to process in a single batch.
    pub batch_size: u32,
    /// Data retention settings, used by the `prune` CLI command.
    #[serde(default)]
    pub retention: RetentionConfig,
    /// URL of the `POST /internal/events` webhook the `load` command notifies
    /// after a successful run. When unset, the `load` command skips hazard
    /// event publishing entirely; the database remains the source of truth
    /// either way. The shared secret is read from `INTERNAL_EVENT_TOKEN`.
    #[serde(default)]
    pub internal_events_url: Option<String>,
}

/// Controls how long historical data is kept before the `prune` command
/// deletes it. Exists to bound storage growth on capacity-limited databases
/// (e.g. Neon free tier's 0.5 GB limit).
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct RetentionConfig {
    /// Delete `approaches` rows whose `close_approach_date` is older than
    /// this many years.
    pub approach_retention_years: u32,
    /// Delete `etl_events` rows whose `started_at` is older than this many
    /// days, while always keeping the most recent `etl_events_keep_min` rows.
    pub etl_event_retention_days: u32,
    /// Minimum number of `etl_events` rows to always keep, regardless of age,
    /// so the dashboard's ETL history panel is never empty.
    pub etl_events_keep_min: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            approach_retention_years: 2,
            etl_event_retention_days: 90,
            etl_events_keep_min: 50,
        }
    }
}

/// Configuration for the HTTP server (Axum).
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct ServerConfig {
    /// Request timeout in seconds.
    pub request_timeout_seconds: u32,
    /// Rate limit: maximum requests allowed per period.
    pub rate_limit_requests: u32,
    /// Rate limit period in seconds.
    pub rate_limit_period_seconds: u64,
    /// Maximum number of concurrent `/api/events/hazards` SSE subscribers.
    /// Each stream is a held task, so this bounds worst-case resource use.
    pub max_hazard_subscribers: usize,
    /// Rate limit for `POST /internal/events`, tighter than the general API
    /// bucket since it's a single trusted caller (the ETL job), not a public
    /// endpoint.
    pub internal_event_rate_limit_requests: u32,
    /// In-memory dashboard response cache settings.
    #[serde(default)]
    pub cache: CacheConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            request_timeout_seconds: 300,
            rate_limit_period_seconds: 60,
            rate_limit_requests: 100,
            max_hazard_subscribers: 100,
            internal_event_rate_limit_requests: 30,
            cache: CacheConfig::default(),
        }
    }
}

/// Controls in-memory TTL caching of read-heavy `DashboardRepository`
/// queries. Missing fields in config fall back
/// to `Default::default()` per-field via the container-level `serde(default)`.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct CacheConfig {
    /// Runtime kill-switch. When `false`, cached wrappers fall through to
    /// the uncached query on every call.
    pub enabled: bool,
    /// TTL in seconds for `get_stats`.
    pub stats_ttl_secs: u32,
    /// TTL in seconds for velocity-data queries.
    pub velocity_ttl_secs: u32,
    /// TTL in seconds for recent/paginated approach queries.
    pub approaches_ttl_secs: u32,
    /// TTL in seconds for ETL run queries.
    pub etl_runs_ttl_secs: u32,
    /// TTL in seconds for catalog listing/detail/similar-asteroid queries.
    pub catalog_ttl_secs: u32,
    /// TTL in seconds for the catalog's distinct orbit/spectral
    /// classification values. Longer-lived: this reference data only
    /// changes when ETL ingests a new classification, unlike the
    /// per-request catalog queries above.
    pub catalog_classifications_ttl_secs: u32,
    /// TTL in seconds for the weekly report summary. Keyed by end date, so
    /// this only needs to outlast one day's worth of requests to avoid
    /// re-aggregating on every dashboard load.
    pub report_ttl_secs: u32,
    /// Maximum entries held by any single keyed (non-singleton) cache, to
    /// bound memory growth from high-cardinality keys (pagination, dates).
    pub max_entries: u32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            stats_ttl_secs: 300,
            velocity_ttl_secs: 600,
            approaches_ttl_secs: 150,
            etl_runs_ttl_secs: 300,
            catalog_ttl_secs: 300,
            catalog_classifications_ttl_secs: 3000,
            report_ttl_secs: 900,
            max_entries: 512,
        }
    }
}

/// Configuration for Prometheus/OTLP metrics.
///
/// Supports both:
/// 1. **OTLP Push**: Push metrics to Grafana Cloud via OTLP endpoint
/// 2. **Prometheus Query**: Query metrics from Prometheus/Grafana for dashboard display
#[derive(Deserialize, Clone, Default)]
pub struct PrometheusConfig {
    /// The OTLP endpoint URL to push metrics to (e.g., Grafana Cloud OTLP gateway).
    /// Example: <https://otlp-gateway-prod-eu-west-2.grafana.net/otlp>
    #[serde(default)]
    pub url: String,
    /// The URL to query metrics from (e.g., Grafana Cloud Prometheus Query API).
    /// Used by /api/metrics/summary endpoint for dashboard display.
    /// Example: <https://prometheus-prod-eu-west-2.grafana.net/api/prom/api/v1/query>
    pub query_url: Option<String>,
    /// The username (or instance ID) for Basic Auth.
    #[serde(default)]
    pub username: String,
    /// The password (or API token) for Basic Auth.
    #[serde(default)]
    pub token: String,
    /// The interval in seconds between OTLP pushes (default: 60).
    #[serde(default = "default_prometheus_interval")]
    pub interval_seconds: u64,
}

/// Configuration for Grafana Cloud Prometheus querying.
///
/// This configuration is specifically for querying metrics from Grafana Cloud Prometheus
/// to display on the dashboard. It uses separate credentials from the OTLP push config.
#[derive(Deserialize, Clone, Default)]
pub struct GrafanaCloudPrometheusConfig {
    /// The Grafana Cloud Prometheus query API URL.
    /// Example: <https://prometheus-prod-eu-west-2.grafana.net/api/prom/api/v1/query>
    #[serde(default)]
    pub url: String,
    /// The Grafana Cloud instance ID (used for Basic Auth username).
    /// This is typically a numeric value from your Grafana Cloud portal.
    #[serde(default)]
    pub instance_id: String,
    /// The Grafana Cloud API token (used for Basic Auth password).
    /// Should have the `metrics:read` permission.
    #[serde(default)]
    pub token: String,
}

impl std::fmt::Debug for GrafanaCloudPrometheusConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GrafanaCloudPrometheusConfig")
            .field("url", &self.url)
            .field("instance_id", &self.instance_id)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

impl GrafanaCloudPrometheusConfig {
    /// Returns true if the configuration is complete and valid.
    pub fn is_valid(&self) -> bool {
        !self.url.is_empty() && !self.instance_id.is_empty() && !self.token.is_empty()
    }

    /// Returns the Basic Auth username (instance ID).
    pub fn username(&self) -> &str {
        &self.instance_id
    }

    /// Returns the Basic Auth password (token).
    pub fn password(&self) -> &str {
        &self.token
    }
}

fn default_prometheus_interval() -> u64 {
    60
}

impl std::fmt::Debug for PrometheusConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrometheusConfig")
            .field("url", &self.url)
            .field("query_url", &self.query_url)
            .field("username", &self.username)
            .field("token", &"[REDACTED]")
            .field("interval_seconds", &self.interval_seconds)
            .finish()
    }
}

impl RustroidSentinelConfig {
    /// Creates a new `RustroidSentinelConfig` instance by loading and merging settings
    /// from multiple sources.
    ///
    /// The configuration is loaded in the following order, with later sources overriding
    /// earlier ones:
    /// 1.  **Base Configuration**: From `config/config.toml`. This file is optional.
    /// 2.  **Environment-Specific Configuration**: From a file corresponding to the
    ///     `RUN_ENV` environment variable (e.g., `config/production.toml`). If `RUN_ENV`
    ///     is not set, it defaults to "development". This file is required.
    /// 3.  **Environment Variables**: Overrides are loaded from environment variables
    ///     prefixed with `SERVICE`. Nested keys are separated by `__`.
    ///
    /// # Example of Environment Variable Override
    /// To override the database URL, you would set an environment variable like this:
    /// `export SERVICE__DATABASE__URL="postgresql://prod:secret@db.example.com/prod_db"`
    ///
    /// # Errors
    /// Returns `Err(Error::Config)` if any part of the loading or deserialization process fails.
    /// The specific cause will be detailed in the wrapped `ConfigError`.
    pub fn new() -> Result<Self, ServiceConfigError> {
        let run_env = env::var("RUN_ENV").unwrap_or_else(|_| "development".into());

        let builder = Config::builder()
            // Base config file is optional
            .add_source(File::with_name("config/config").required(false))
            // Env-specific config file is required
            .add_source(File::with_name(&format!("config/{}", run_env)).required(true))
            // Load env vars with prefix `SERVICE` and separator `__`
            .add_source(Environment::with_prefix("SERVICE").separator("__"));

        let config = builder.build().map_err(|err| match err {
            config::ConfigError::FileParse { uri, .. } => {
                ServiceConfigError::FileParse(uri.unwrap_or_else(|| "Unknown file".to_string()))
            }
            config::ConfigError::Message(msg) if msg.contains("not found") => {
                ServiceConfigError::MissingFile(msg)
            }
            config::ConfigError::Type { key, .. } => {
                ServiceConfigError::Deserialize(format!("Invalid type for key '{:?}' when building configuration (e.g., from environment variables).", key))
            }
            _ => ServiceConfigError::Unexpected(err.to_string()),
        })?;

        let settings = config.try_deserialize().map_err(|err| match err {
            config::ConfigError::NotFound(key) => ServiceConfigError::Deserialize(format!(
                "Missing required configuration key: {}",
                key
            )),
            config::ConfigError::Type {
                key,
                unexpected,
                expected,
                ..
            } => ServiceConfigError::Deserialize(format!(
                "Invalid type for key '{:?}': expected {}, but found {}",
                key, expected, unexpected
            )),
            _ => ServiceConfigError::Deserialize(err.to_string()),
        })?;

        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        assert_eq!(default_version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(default_max_concurrent_requests(), 5);
        assert!(default_enable_gzip());
        let server_config = ServerConfig::default();
        assert_eq!(server_config.request_timeout_seconds, 300);
        assert_eq!(server_config.rate_limit_requests, 100);
        assert_eq!(server_config.rate_limit_period_seconds, 60);
        assert_eq!(default_prometheus_interval(), 60);
    }

    #[test]
    fn test_grafana_cloud_prometheus_config() {
        let mut config = GrafanaCloudPrometheusConfig::default();
        assert!(!config.is_valid());

        config.url = "http://localhost".to_string();
        config.instance_id = "test_user".to_string();
        config.token = "test_pass".to_string();

        assert!(config.is_valid());
        assert_eq!(config.username(), "test_user");
        assert_eq!(config.password(), "test_pass");
    }

    #[test]
    fn test_debug_implementations_redact_secrets() {
        let db = DatabaseConfig {
            url: "secret_url".to_string(),
            listen_url: None,
            max_connections: 5,
            min_connections: 1,
            connect_timeout_seconds: 10,
        };
        let db_debug = format!("{:?}", db);
        assert!(db_debug.contains("[REDACTED]"));
        assert!(!db_debug.contains("secret_url"));

        let nasa = NasaConfig {
            api_key: "secret_key".to_string(),
            base_url: "url".to_string(),
            timeout_seconds: 1,
            max_retries: 1,
            retry_delay_ms: 1,
            max_concurrent_requests: 1,
        };
        let nasa_debug = format!("{:?}", nasa);
        assert!(nasa_debug.contains("[REDACTED]"));
        assert!(!nasa_debug.contains("secret_key"));

        let discord = DiscordConfig {
            webhook_url: "secret_webhook".to_string(),
            timeout_seconds: 1,
            max_retries: 1,
        };
        let discord_debug = format!("{:?}", discord);
        assert!(discord_debug.contains("[REDACTED]"));
        assert!(!discord_debug.contains("secret_webhook"));

        let prom = PrometheusConfig {
            url: "url".to_string(),
            query_url: None,
            username: "user".to_string(),
            token: "secret_token".to_string(),
            interval_seconds: 60,
        };
        let prom_debug = format!("{:?}", prom);
        assert!(prom_debug.contains("[REDACTED]"));
        assert!(!prom_debug.contains("secret_token"));

        let grafana = GrafanaCloudPrometheusConfig {
            url: "url".to_string(),
            instance_id: "id".to_string(),
            token: "secret_token".to_string(),
        };
        let grafana_debug = format!("{:?}", grafana);
        assert!(grafana_debug.contains("[REDACTED]"));
        assert!(!grafana_debug.contains("secret_token"));
    }
}
