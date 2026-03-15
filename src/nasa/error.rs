//! # NASA API Error Types
//!
//! This module defines error types specifically for NASA API interactions.

use thiserror::Error;

/// Errors specifically related to interacting with NASA APIs.
///
/// This enum encapsulates all possible errors that can occur when
/// communicating with NASA's NeoWs API.
///
/// # Variants
///
/// * `HttpRequest` - Network-level failures (timeout, connection refused, etc.)
/// * `Deserialization` - Response body couldn't be parsed
/// * `ApiError` - API returned an error status code
#[derive(Error, Debug)]
pub enum NasaApiError {
    /// An error occurred during the HTTP request or connection phase.
    ///
    /// This includes network failures, timeouts, DNS resolution errors,
    /// and middleware errors from reqwest.
    #[error("HTTP request failed: {0}")]
    HttpRequest(#[from] reqwest_middleware::Error),

    /// The API responded successfully, but the body could not be deserialized.
    ///
    /// This typically indicates a schema mismatch between the expected
    /// response format and what the API actually returned.
    #[error("Failed to deserialize response: {0}")]
    Deserialization(#[from] reqwest::Error),

    /// The API returned a non-success status code or a structured error message.
    ///
    /// Contains the error body returned by the API for debugging.
    #[error("API returned an error: {0}")]
    ApiError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_display() {
        let err = NasaApiError::ApiError("Rate Limit Exceeded".to_string());
        assert_eq!(
            err.to_string(),
            "API returned an error: Rate Limit Exceeded"
        );
    }
}
