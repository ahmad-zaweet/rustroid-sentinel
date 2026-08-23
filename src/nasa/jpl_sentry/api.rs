//! Client for JPL's Sentry impact-monitoring API.
//!
//! This module abstracts requests to `https://ssd-api.jpl.nasa.gov/sentry.api`,
//! which — unlike NeoWs — requires no API key and exposes no rate-limit
//! headers to react to.

use crate::api::client::SharedHttpClient;
use crate::nasa::error::NasaApiError;
use crate::nasa::jpl_sentry::responses::{SentryLookupResponse, SentrySummary};
use crate::settings::JplSentryConfig;
use reqwest::Url;
use tracing::{debug, error, instrument, warn};

/// A client for JPL's Sentry impact-monitoring API.
#[derive(Clone)]
pub struct JplSentryApi {
    http_client: SharedHttpClient,
    config: JplSentryConfig,
}

impl JplSentryApi {
    /// Creates a new `JplSentryApi` client.
    pub fn new(http_client: SharedHttpClient, config: JplSentryConfig) -> Self {
        debug!(base_url = %config.base_url, "Initialized JPL Sentry API client.");
        Self {
            http_client,
            config,
        }
    }

    /// Looks up a single asteroid by its SPK-ID (NASA's `neo_reference_id`).
    ///
    /// Returns `Ok(None)` if the object isn't a currently tracked virtual
    /// impactor (Sentry's `error` response) — this is the expected outcome
    /// for the overwhelming majority of asteroids, not a failure.
    ///
    /// # Errors
    ///
    /// Returns `Err(NasaApiError)` for network failures, non-success HTTP
    /// statuses, or a response body that doesn't match the expected shape.
    #[instrument(skip(self), fields(spk_id = %spk_id))]
    pub async fn lookup_by_spk(&self, spk_id: &str) -> Result<Option<SentrySummary>, NasaApiError> {
        let client = self.http_client.http_client();
        let url = Url::parse_with_params(&self.config.base_url, &[("spk", spk_id)])
            .map_err(|e| NasaApiError::HttpRequest(anyhow::anyhow!(e).into()))?;

        let response: reqwest::Response = client.get(url).send().await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .map_err(|e| NasaApiError::HttpRequest(e.into()))?;
            error!(
                status = %status,
                spk_id = %spk_id,
                error_body = %error_body,
                "Sentry lookup failed"
            );
            return Err(NasaApiError::ApiError(error_body));
        }

        let body = response.json::<SentryLookupResponse>().await?;

        if let Some(reason) = &body.error {
            debug!(spk_id = %spk_id, reason = %reason, "Not a current Sentry virtual impactor");
            return Ok(None);
        }

        match body.summary {
            Some(summary) => {
                debug!(
                    spk_id = %spk_id,
                    torino = ?summary.ts_max,
                    palermo = ?summary.ps_cum,
                    "Found Sentry virtual impactor"
                );
                Ok(Some(summary))
            }
            None => {
                warn!(spk_id = %spk_id, "Sentry response had neither summary nor error");
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_test_config(base_url: String) -> JplSentryConfig {
        JplSentryConfig {
            base_url,
            request_delay_ms: 0,
            stale_days: 30,
        }
    }

    async fn create_test_client(mock_server: &MockServer) -> JplSentryApi {
        let config = create_test_config(mock_server.uri());
        let http_client = SharedHttpClient::new_for_test(mock_server.uri().to_string()).await;
        JplSentryApi::new(http_client, config)
    }

    #[tokio::test]
    async fn test_lookup_matched() -> anyhow::Result<()> {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(query_param("spk", "2099942"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"summary": {"des": "99942", "ps_cum": "-3.32", "ts_max": "0", "ip": "2.7e-06"}}"#,
            ))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;
        let result = client.lookup_by_spk("2099942").await?;

        let summary = result.expect("expected a matched summary");
        assert_eq!(summary.des, "99942");
        assert_eq!(summary.ts_max, Some(0));

        Ok(())
    }

    #[tokio::test]
    async fn test_lookup_not_found_returns_none() -> anyhow::Result<()> {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(query_param("spk", "9999999"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"error": "specified object not found"}"#),
            )
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;
        let result = client.lookup_by_spk("9999999").await?;

        assert!(result.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_lookup_http_error() -> anyhow::Result<()> {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(400).set_body_string(r#"{"error": "bad request"}"#))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;
        let result = client.lookup_by_spk("123").await;

        assert!(result.is_err());

        Ok(())
    }
}
