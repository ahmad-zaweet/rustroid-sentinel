//! Client for JPL's Small-Body Database (SBDB) API.
//!
//! This module abstracts requests to `https://ssd-api.jpl.nasa.gov/sbdb.api`,
//! which — like Sentry — requires no API key and exposes no rate-limit
//! headers to react to. Unlike Sentry, an unmatched object comes back as an
//! HTTP 400 with a `message` body rather than an HTTP 200 with an `error`
//! field, so that case is handled before the generic error path.

use crate::api::client::SharedHttpClient;
use crate::nasa::error::NasaApiError;
use crate::nasa::jpl_sbdb::responses::{SbdbOrbitSummary, SbdbResponse};
use crate::settings::JplSbdbConfig;
use reqwest::Url;
use tracing::{debug, error, instrument, warn};

/// A client for JPL's Small-Body Database API.
#[derive(Clone)]
pub struct JplSbdbApi {
    http_client: SharedHttpClient,
    config: JplSbdbConfig,
}

impl JplSbdbApi {
    /// Creates a new `JplSbdbApi` client.
    pub fn new(http_client: SharedHttpClient, config: JplSbdbConfig) -> Self {
        debug!(base_url = %config.base_url, "Initialized JPL SBDB API client.");
        Self {
            http_client,
            config,
        }
    }

    /// Looks up a single asteroid's orbital elements by its SPK-ID (NASA's
    /// `neo_reference_id`).
    ///
    /// Returns `Ok(None)` if SBDB has no record for the object, or if it has
    /// a record with no fitted orbit — both treated as "nothing to persist"
    /// rather than a failure.
    ///
    /// # Errors
    ///
    /// Returns `Err(NasaApiError)` for network failures, non-"not found"
    /// error statuses, or a response body that doesn't match the expected shape.
    #[instrument(skip(self), fields(spk_id = %spk_id))]
    pub async fn lookup_by_spk(
        &self,
        spk_id: &str,
    ) -> Result<Option<SbdbOrbitSummary>, NasaApiError> {
        let client = self.http_client.http_client();
        let url =
            Url::parse_with_params(&self.config.base_url, &[("spk", spk_id), ("phys-par", "1")])
                .map_err(|e| NasaApiError::HttpRequest(anyhow::anyhow!(e).into()))?;

        let response: reqwest::Response = client.get(url).send().await?;

        let status = response.status();
        let body_text = response
            .text()
            .await
            .map_err(|e| NasaApiError::HttpRequest(e.into()))?;

        if !status.is_success() {
            if let Ok(body) = serde_json::from_str::<SbdbResponse>(&body_text)
                && body.message.is_some()
            {
                debug!(spk_id = %spk_id, message = ?body.message, "SBDB has no record for this object");
                return Ok(None);
            }
            error!(status = %status, spk_id = %spk_id, error_body = %body_text, "SBDB lookup failed");
            return Err(NasaApiError::ApiError(body_text));
        }

        let body: SbdbResponse = serde_json::from_str(&body_text)?;

        match body.to_orbit_summary() {
            Some(summary) => {
                debug!(spk_id = %spk_id, "Found SBDB orbit record");
                Ok(Some(summary))
            }
            None => {
                warn!(spk_id = %spk_id, message = ?body.message, "SBDB response had no orbit");
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

    fn create_test_config(base_url: String) -> JplSbdbConfig {
        JplSbdbConfig {
            base_url,
            request_delay_ms: 0,
            stale_days: 90,
        }
    }

    async fn create_test_client(mock_server: &MockServer) -> JplSbdbApi {
        let config = create_test_config(mock_server.uri());
        let http_client = SharedHttpClient::new_for_test(mock_server.uri().to_string()).await;
        JplSbdbApi::new(http_client, config)
    }

    #[tokio::test]
    async fn test_lookup_matched() -> anyhow::Result<()> {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(query_param("spk", "2099942"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{
                    "object": {"orbit_class": {"name": "Aten", "code": "ATE"}},
                    "orbit": {"elements": [{"name": "e", "value": "0.1914"}]}
                }"#,
            ))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;
        let result = client.lookup_by_spk("2099942").await?;

        let summary = result.expect("expected an orbit summary");
        assert_eq!(summary.eccentricity, Some(0.1914));
        assert_eq!(summary.orbit_class.as_deref(), Some("Aten"));

        Ok(())
    }

    #[tokio::test]
    async fn test_lookup_not_found_returns_none() -> anyhow::Result<()> {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(query_param("spk", "9999999"))
            .respond_with(
                ResponseTemplate::new(400).set_body_string(
                    r#"{"message": "specified object was not found", "code": "400"}"#,
                ),
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
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;
        let result = client.lookup_by_spk("123").await;

        assert!(result.is_err());

        Ok(())
    }
}
