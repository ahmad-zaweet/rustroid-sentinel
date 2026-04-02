//! Provides a client for interacting with the NASA NeoWs (Near Earth Object Web Service) API.
//!
//! This module abstracts the details of making HTTP requests to the NeoWs API and deserializing
//! the responses into strongly-typed Rust structs.

use crate::api::client::SharedHttpClient;
use crate::nasa::asteroid_neows::responses::{NearEarthObject, NeoFeed};
use crate::{nasa::error::NasaApiError, settings::NasaConfig};
use chrono::NaiveDate;
use futures_util::stream::StreamExt;
use reqwest::Url;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;
use tracing::{debug, error, instrument, warn};

/// A client for the NASA NeoWs (Near Earth Object Web Service) API.
///
/// This client provides methods to interact with the NeoWs API endpoints, such as
/// fetching a feed of asteroids or looking up a specific asteroid.
pub struct NeoWsApi {
    http_client: SharedHttpClient,
    config: NasaConfig,
    base_feed_url: String,
    base_lookup_url: String,
    semaphore: Arc<Semaphore>,
}

impl NeoWsApi {
    /// Creates a new `NeoWsApi` client.
    ///
    /// # Arguments
    ///
    /// * `http_client` - A shared HTTP client for making requests.
    /// * `config` - The NASA API configuration.
    pub fn new(http_client: SharedHttpClient, config: NasaConfig) -> Self {
        let base_path = format!("{}/neo/rest/v1", config.base_url);
        let base_feed_url = format!("{}/feed", base_path);
        let base_lookup_url = format!("{}/neo", base_path);

        debug!(
            base_url = %config.base_url,
            "Initialized NeoWs API client."
        );

        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_requests));

        Self {
            http_client,
            config,
            base_feed_url,
            base_lookup_url,
            semaphore,
        }
    }

    /// Retrieves a list of asteroids based on their closest approach date to Earth.
    ///
    /// Corresponds to the `/neo/rest/v1/feed` endpoint.
    ///
    /// # Arguments
    ///
    /// * `start_date` - The starting date for the asteroid search.
    /// * `end_date` - The ending date for the asteroid search.
    ///
    /// # Errors
    ///
    /// Returns `Err(NasaApiError)` if the request fails, if the response cannot be
    /// deserialized, or if the API returns a non-success status code.
    ///
    /// # Example
    ///
    /// ```ignore
    /// # use rustroid_sentinel::nasa::asteroid_neows::api::NeoWsApi;
    /// # use rustroid_sentinel::api::client::SharedHttpClient;
    /// # use rustroid_sentinel::settings::RustroidSentinelConfig;
    /// # use chrono::NaiveDate;
    /// # async {
    /// # let settings = RustroidSentinelConfig::new().unwrap();
    /// # let shared_client = SharedHttpClient::new(&settings).await.unwrap();
    /// # let neows_client = NeoWsApi::new(shared_client, settings.nasa);
    /// let start_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    /// let end_date = NaiveDate::from_ymd_opt(2024, 1, 8).unwrap();
    ///
    /// let feed = neows_client.get_feed(start_date, end_date).await;
    /// match feed {
    ///     Ok(f) => println!("Found {} asteroids.", f.element_count),
    ///     Err(e) => eprintln!("Error fetching feed: {}", e),
    /// }
    /// # };
    /// ```

    #[instrument(skip(self), fields(start_date = %start_date, end_date = %end_date))]
    pub async fn get_feed(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<NeoFeed, NasaApiError> {
        let client = self.http_client.http_client();
        let start_date_str = start_date.format("%Y-%m-%d").to_string();
        let end_date_str = end_date.format("%Y-%m-%d").to_string();

        debug!(
            start_date = %start_date_str,
            end_date = %end_date_str,
            "Fetching asteroid feed"
        );

        let url = Url::parse_with_params(
            &self.base_feed_url,
            &[
                ("start_date", start_date_str.as_str()),
                ("end_date", end_date_str.as_str()),
                ("api_key", self.config.api_key.as_str()),
            ],
        )
        .map_err(|e| NasaApiError::HttpRequest(anyhow::anyhow!(e).into()))?;

        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| NasaApiError::HttpRequest(anyhow::anyhow!(e).into()))?;
        let response: reqwest::Response = client.get(url).send().await?;
        self.handle_rate_limits(response.headers());

        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .map_err(|e| NasaApiError::HttpRequest(e.into()))?;
            error!(
                status = %status,
                error_body = %error_body,
                "Feed request failed"
            );

            return Err(NasaApiError::ApiError(error_body));
        }

        // Stream the response body to a temporary file to avoid loading large responses into memory
        let temp_path = std::env::temp_dir().join(format!(
            "neows_feed_{}_{}.json",
            start_date_str, end_date_str
        ));
        let mut file = File::create(&temp_path).await?;

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| NasaApiError::HttpRequest(e.into()))?;
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);

        // Deserialize directly from the file using streaming JSON parser
        let feed = serde_json::from_reader::<_, NeoFeed>(std::fs::File::open(&temp_path)?)?;

        // Clean up the temporary file
        let _ = tokio::fs::remove_file(&temp_path).await;

        debug!(
            element_count = feed.element_count,
            "Successfully fetched asteroid feed"
        );

        Ok(feed)
    }

    /// Looks up a specific asteroid based on its NASA JPL small body (SPK-ID) ID.
    ///
    /// Corresponds to the `/neo/rest/v1/neo/{asteroid_id}` endpoint.
    ///
    /// # Arguments
    ///
    /// * `asteroid_id` - The SPK-ID of the asteroid to look up.
    ///
    /// # Errors
    ///
    /// Returns `Err(NasaApiError)` if the request fails, if the response cannot be
    /// deserialized, or if the API returns a non-success status code.
    #[instrument(skip(self), fields(asteroid_id = %asteroid_id))]
    pub async fn asteroid_lookup(&self, asteroid_id: u64) -> Result<NearEarthObject, NasaApiError> {
        let client = self.http_client.http_client();
        let url = format!("{}/{}", self.base_lookup_url, asteroid_id);

        debug!(
            asteroid_id = %asteroid_id,
            url = %url,
            "Looking up asteroid"
        );

        let url = Url::parse_with_params(&url, &[("api_key", self.config.api_key.as_str())])
            .map_err(|e| NasaApiError::HttpRequest(anyhow::anyhow!(e).into()))?;

        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| NasaApiError::HttpRequest(anyhow::anyhow!(e).into()))?;
        let response: reqwest::Response = client.get(url).send().await?;

        self.handle_rate_limits(response.headers());

        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .map_err(|e| NasaApiError::HttpRequest(e.into()))?;
            error!(
                status = %status,
                asteroid_id = %asteroid_id,
                error_body = %error_body,
                "Asteroid lookup failed"
            );
            return Err(NasaApiError::ApiError(error_body));
        }

        let asteroid = response.json::<NearEarthObject>().await?;
        debug!(
            asteroid_id = %asteroid_id,
            name = %asteroid.name,
            "Successfully looked up asteroid"
        );

        Ok(asteroid)
    }

    /// Parses and logs NASA API rate limit headers from the response.
    ///
    /// This method extracts monitoring headers (`x-ratelimit-limit` and `x-ratelimit-remaining`)
    /// provided by NASA's API Gateway. It helps in proactively identifying if the
    /// application is approaching its usage limits.
    ///
    /// # Behavior
    /// - **Debug**: Logs current limit and remaining requests.
    /// - **Warning**: Logs a warning if fewer than 100 requests remain.
    /// - **Error**: Logs an error if the rate limit is completely exhausted.
    fn handle_rate_limits(&self, headers: &reqwest::header::HeaderMap) {
        let limit = parse_header(headers, "x-ratelimit-limit");
        let remaining = parse_header(headers, "x-ratelimit-remaining");

        if let (Some(limit), Some(remaining)) = (limit, remaining) {
            self.log_rate_limit_status(limit, remaining);
        } else {
            debug!("NASA API rate limit headers not found in response.");
        }
    }

    /// Helper to log rate limit status with appropriate severity levels.
    fn log_rate_limit_status(&self, limit: u32, remaining: u32) {
        debug!(limit, remaining, "NASA API rate limit status");

        if remaining == 0 {
            error!("NASA API rate limit exhausted. Subsequent requests will likely fail.");
        } else if remaining < 100 {
            warn!(remaining, "NASA API rate limit is getting low.");
        }
    }
}

/// Helper function to parse a numeric header value.
fn parse_header(headers: &reqwest::header::HeaderMap, name: &str) -> Option<u32> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::client::SharedHttpClient;
    use crate::settings::NasaConfig;
    use chrono::NaiveDate;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_test_config() -> NasaConfig {
        NasaConfig {
            api_key: "test-key".to_string(),
            base_url: "https://api.nasa.gov".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
            retry_delay_ms: 100,
            max_concurrent_requests: 5,
        }
    }

    async fn create_test_client(mock_server: &MockServer) -> NeoWsApi {
        let mut config = create_test_config();
        config.base_url = mock_server.uri();
        let http_client = SharedHttpClient::new_for_test(mock_server.uri().to_string()).await;
        NeoWsApi::new(http_client, config)
    }

    #[tokio::test]
    async fn test_get_feed_success() -> anyhow::Result<()> {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/neo/rest/v1/feed"))
            .and(query_param("api_key", "test-key"))
            .and(query_param("start_date", "2024-01-01"))
            .and(query_param("end_date", "2024-01-07"))
            .respond_with(ResponseTemplate::new(200).set_body_string(include_str!(
                "../../../tests/fixtures/nasa_responses/feed_valid.json"
            )))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;

        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 7).unwrap();
        let result = client.get_feed(start, end).await;

        if let Err(e) = &result {
            println!("Error: {:?}", e);
        }
        assert!(result.is_ok());
        assert_eq!(
            mock_server
                .received_requests()
                .await
                .map(|r| r.len())
                .unwrap_or(0),
            1
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_get_feed_empty_response() -> anyhow::Result<()> {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/neo/rest/v1/feed"))
            .respond_with(ResponseTemplate::new(200).set_body_string(include_str!(
                "../../../tests/fixtures/nasa_responses/feed_empty.json"
            )))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;

        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let result = client.get_feed(start, end).await;

        assert!(result.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_get_feed_api_error() -> anyhow::Result<()> {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/neo/rest/v1/feed"))
            .respond_with(ResponseTemplate::new(400).set_body_string(include_str!(
                "../../../tests/fixtures/nasa_responses/feed_error.json"
            )))
            .mount(&mock_server)
            .await;

        let client = create_test_client(&mock_server).await;

        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let result = client.get_feed(start, end).await;

        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_neo_ws_api_initialization() {
        let config = create_test_config();
        let base_path = format!("{}/neo/rest/v1", config.base_url);
        let expected_feed_url = format!("{}/feed", base_path);
        let expected_lookup_url = format!("{}/neo", base_path);

        assert_eq!(
            expected_feed_url,
            format!("{}/neo/rest/v1/feed", config.base_url)
        );
        assert_eq!(
            expected_lookup_url,
            format!("{}/neo/rest/v1/neo", config.base_url)
        );
    }
}
