//! # Mock Servers
//!
//! Provides mock server implementations for external services
//! (NASA API, Discord webhooks) using wiremock.

use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

/// Mock NASA API server.
pub struct MockNasaServer {
    server: MockServer,
}

impl MockNasaServer {
    /// Starts a new mock NASA server.
    pub async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let server = MockServer::start().await;
        Ok(Self { server })
    }

    /// Returns the server URI.
    pub fn uri(&self) -> String {
        self.server.uri()
    }

    /// Configures the server to return a successful feed response.
    pub async fn mount_successful_feed(&self, start_date: &str, end_date: &str) {
        Mock::given(method("GET"))
            .and(path("/neo/rest/v1/feed"))
            .and_param("api_key", "test-key")
            .and_param("start_date", start_date)
            .and_param("end_date", end_date)
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({
                    "links": {
                        "next": null
                    },
                    "element_count": 1,
                    "near_earth_objects": {
                        start_date: [
                            {
                                "links": {
                                    "self": "https://api.nasa.gov/neo/rest/v1/neo/20240101?api_key=test-key"
                                },
                                "id": "20240101",
                                "neo_reference_id": "20240101",
                                "name": "Test Asteroid",
                                "nasa_jpl_url": "https://ssd.jpl.nasa.gov/tools/sbdb_lookup.html#/?sstr=20240101",
                                "absolute_magnitude_h": 22.5,
                                "estimated_diameter": {
                                    "kilometers": {
                                        "estimated_diameter_min": 0.1,
                                        "estimated_diameter_max": 0.5
                                    }
                                },
                                "is_potentially_hazardous_asteroid": false,
                                "is_sentry_object": false,
                                "close_approach_data": [
                                    {
                                        "close_approach_date_full": "2024-06-15",
                                        "epoch_date_close_approach": 1718409600000,
                                        "relative_velocity": {
                                            "kilometers_per_hour": "72000"
                                        },
                                        "miss_distance": {
                                            "kilometers": "500000"
                                        },
                                        "orbiting_body": "Earth"
                                    }
                                ]
                            }
                        ]
                    }
                })))
            .mount(&self.server)
            .await;
    }

    /// Configures the server to return an error response.
    pub async fn mount_error_response(&self, status: u16) {
        Mock::given(method("GET"))
            .and(path("/neo/rest/v1/feed"))
            .respond_with(ResponseTemplate::new(status).set_body_json(json!({
                "error": {
                    "code": "ERROR",
                    "message": "Test error message"
                }
            })))
            .mount(&self.server)
            .await;
    }

    /// Configures the server to return a rate limit response.
    pub async fn mount_rate_limit(&self, retry_after: u64) {
        Mock::given(method("GET"))
            .and(path("/neo/rest/v1/feed"))
            .respond_with(
                ResponseTemplate::new(429).insert_header("Retry-After", retry_after.to_string()),
            )
            .mount(&self.server)
            .await;
    }

    /// Returns the number of requests received.
    pub async fn received_requests(&self) -> usize {
        self.server
            .received_requests()
            .await
            .map(|reqs| reqs.len())
            .unwrap_or(0)
    }
}

/// Mock Discord webhook server.
pub struct MockDiscordServer {
    server: MockServer,
}

impl MockDiscordServer {
    /// Starts a new mock Discord server.
    pub async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let server = MockServer::start().await;
        Ok(Self { server })
    }

    /// Returns the server URI (webhook URL).
    pub fn webhook_url(&self) -> String {
        format!("{}/webhook", self.server.uri())
    }

    /// Configures the server to accept webhook requests.
    pub async fn mount_success(&self) {
        Mock::given(method("POST"))
            .and(path("/webhook"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&self.server)
            .await;
    }

    /// Configures the server to return an error.
    pub async fn mount_error(&self, status: u16) {
        Mock::given(method("POST"))
            .and(path("/webhook"))
            .respond_with(ResponseTemplate::new(status))
            .mount(&self.server)
            .await;
    }

    /// Returns the number of webhook requests received.
    pub async fn received_requests(&self) -> usize {
        self.server
            .received_requests()
            .await
            .map(|reqs| reqs.len())
            .unwrap_or(0)
    }

    /// Returns the last webhook payload.
    pub async fn last_webhook_payload(&self) -> Option<serde_json::Value> {
        self.server
            .received_requests()
            .await
            .and_then(|reqs| reqs.last())
            .and_then(|req| serde_json::from_slice(req.body.as_slice()).ok())
    }
}
