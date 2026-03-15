//! # NASA NeoWs (Near Earth Object Web Service) Integration
//!
//! This module provides a high-level client for interacting with NASA's NeoWs API,
//! which delivers data on near-Earth objects (NEOs) and their close approaches to Earth.
//!
//! ## Submodules
//!
//! - [`api`]: Contains the [`NeoWsApi`](api::NeoWsApi) client for making API requests
//! - [`responses`]: Data structures for deserializing API responses
//!
//! ## Features
//!
//! - **Rate Limiting**: Respects NASA API rate limits with header monitoring
//! - **Concurrency Control**: Limits concurrent requests via semaphore
//! - **Retry Logic**: Automatic retries with exponential backoff (via HTTP client)
//! - **Strong Typing**: Fully typed response structures
//!
//! ## Example
//!
//! ```rust,no_run
//! # use rustroid_sentinel::nasa::asteroid_neows::api::NeoWsApi;
//! # use rustroid_sentinel::api::client::SharedHttpClient;
//! # use rustroid_sentinel::settings::RustroidSentinelConfig;
//! # use chrono::NaiveDate;
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = RustroidSentinelConfig::new()?;
//! let http_client = SharedHttpClient::new(&config).await?;
//! let neows = NeoWsApi::new(http_client, config.nasa.clone());
//!
//! let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
//! let end = NaiveDate::from_ymd_opt(2024, 1, 7).unwrap();
//!
//! let feed = neows.get_feed(start, end).await?;
//! println!("Retrieved {} asteroids", feed.element_count);
//! # Ok(())
//! # }
//! ```
//!
//! ## API Endpoints
//!
//! | Endpoint | Method | Description |
//! |----------|--------|-------------|
//! | `/feed` | GET | Fetch NEOs by date range |
//! | `/neo/{id}` | GET | Lookup specific NEO by ID |
//!
//! ## Rate Limits
//!
//! The NASA API enforces rate limits communicated via response headers:
//! - `x-ratelimit-limit`: Maximum requests per hour
//! - `x-ratelimit-remaining`: Remaining requests in current window
//!
//! The client logs warnings when fewer than 100 requests remain.

pub mod api;
pub mod responses;
