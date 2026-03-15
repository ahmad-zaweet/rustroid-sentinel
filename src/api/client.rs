//! # HTTP Client
//!
//! This module provides a shared HTTP client with retry middleware for external API communication.
//! It uses [`reqwest`] as the underlying HTTP library with [`reqwest_middleware`] for extensibility
//! and [`reqwest_retry`] for automatic retry with exponential backoff.

use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use std::{sync::Arc, time::Duration};

use crate::{error::HttpClientError, settings::RustroidSentinelConfig};

/// A shared HTTP client with retry middleware.
///
/// This client is designed to be cloned and shared across threads safely.
/// It includes:
/// - Configurable timeouts
/// - Gzip compression support
/// - Custom User-Agent
/// - Exponential backoff retry policy for transient failures
///
/// # Thread Safety
///
/// This client is `Clone` and `Send + Sync` safe, using `Arc` internally
/// to share the underlying client instance.
#[derive(Clone)]
pub struct SharedHttpClient {
    http_client: Arc<ClientWithMiddleware>,
}

impl SharedHttpClient {
    /// Creates a new shared HTTP client from the application configuration.
    ///
    /// The client is configured with:
    /// - Connection and pool timeouts from [`HttpConfig`](crate::settings::HttpConfig)
    /// - Gzip compression enabled/disabled based on config
    /// - Custom User-Agent string
    /// - Exponential backoff retry policy with configurable max retries
    /// - TLS certificate verification enforced (no invalid certs accepted)
    /// - Limited redirect policy (max 5 redirects)
    ///
    /// # Arguments
    ///
    /// * `configuration` - The root application configuration containing HTTP settings.
    ///
    /// # Errors
    ///
    /// Returns [`HttpClientError::HttpClientBuild`] if the underlying reqwest client
    /// fails to build (e.g., invalid TLS configuration).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use rustroid_sentinel::api::client::SharedHttpClient;
    /// use rustroid_sentinel::settings::RustroidSentinelConfig;
    ///
    /// # async fn example(config: RustroidSentinelConfig) -> Result<(), rustroid_sentinel::error::HttpClientError> {
    /// let client = SharedHttpClient::new(&config).await?;
    /// let response = client.http_client()
    ///     .get("https://api.nasa.gov/neo/rest/v1/feed")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(
        configuration: &RustroidSentinelConfig,
    ) -> Result<SharedHttpClient, HttpClientError> {
        let base_http_client_result = Client::builder()
            .timeout(Duration::from_secs(
                configuration.http.connect_timeout_seconds,
            ))
            .connect_timeout(Duration::from_secs(
                configuration.http.connect_timeout_seconds,
            ))
            .pool_idle_timeout(Duration::from_secs(
                configuration.http.pool_idle_timeout_seconds,
            ))
            .gzip(configuration.http.enable_gzip)
            .user_agent(&configuration.http.user_agent)
            // Security: Enforce TLS certificate verification
            .danger_accept_invalid_certs(false)
            .danger_accept_invalid_hostnames(false)
            // Security: Limit redirects to prevent open redirect loops
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| {
                HttpClientError::HttpClientBuild(format!(
                    "Failed to build client due to an error: {}",
                    e
                ))
            });

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(
                Duration::from_millis(200),
                Duration::from_millis(configuration.nasa.retry_delay_ms),
            )
            .build_with_max_retries(configuration.nasa.max_retries);

        let base_http_client = base_http_client_result?;
        let http_client = ClientBuilder::new(base_http_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        Ok(Self {
            http_client: Arc::new(http_client),
        })
    }

    /// Returns a reference to the underlying HTTP client.
    ///
    /// Use this method to access the full reqwest API for making requests.
    pub fn http_client(&self) -> &Arc<ClientWithMiddleware> {
        &self.http_client
    }

    /// Creates a test HTTP client pointing to a mock server.
    ///
    /// This is a test-only helper for creating clients without full configuration.
    #[cfg(test)]
    pub async fn new_for_test(_base_url: String) -> Self {
        use std::time::Duration;

        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(2))
            .build()
            .expect("Failed to create test client");

        let client_with_middleware = ClientBuilder::new(client).build();

        Self {
            http_client: Arc::new(client_with_middleware),
        }
    }
}
