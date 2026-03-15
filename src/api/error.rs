//! # API Error Types
//!
//! This module defines error types for API operations.
//! It provides structured errors for request handling, response generation,
//! and HTTP-specific error conditions.

use thiserror::Error;

/// Errors that can occur during API operations.
#[derive(Error, Debug)]
pub enum ApiError {
    /// Database operation failed while handling API request.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Failed to serialize response.
    #[error("failed to serialize response: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Invalid query parameters.
    #[error("invalid query parameter '{param}': {reason}")]
    InvalidQuery {
        /// The name of the invalid parameter.
        param: String,
        /// Explanation of why the parameter is invalid.
        reason: String,
    },

    /// Resource not found.
    #[error("resource not found: {0}")]
    NotFound(String),

    /// Invalid request body.
    #[error("invalid request body: {0}")]
    InvalidBody(String),

    /// Authentication required.
    #[error("authentication required")]
    Unauthorized,

    /// Insufficient permissions.
    #[error("insufficient permissions")]
    Forbidden,

    /// Request timeout.
    #[error("request timed out after {timeout:?}")]
    Timeout {
        /// The timeout duration that was exceeded.
        timeout: std::time::Duration,
    },

    /// Internal server error.
    #[error("internal server error: {0}")]
    Internal(String),

    /// Service temporarily unavailable.
    #[error("service temporarily unavailable: {0}")]
    Unavailable(String),

    /// Metrics operation failed.
    #[error("metrics error: {0}")]
    Metrics(#[from] crate::metrics::error::MetricsError),
}

impl ApiError {
    /// Returns the HTTP status code for this error.
    pub fn status_code(&self) -> axum::http::StatusCode {
        match self {
            Self::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            Self::InvalidQuery { .. } | Self::InvalidBody(_) => axum::http::StatusCode::BAD_REQUEST,
            Self::Unauthorized => axum::http::StatusCode::UNAUTHORIZED,
            Self::Forbidden => axum::http::StatusCode::FORBIDDEN,
            Self::Timeout { .. } => axum::http::StatusCode::GATEWAY_TIMEOUT,
            Self::Unavailable(_) => axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Self::Database(_) | Self::Serialization(_) | Self::Internal(_) | Self::Metrics(_) => {
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    /// Returns true if the error is retryable.
    ///
    /// Timeouts and service unavailable errors are considered retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Timeout { .. } | Self::Unavailable(_))
    }

    /// Returns true if the error is a client error (4xx).
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            Self::NotFound(_)
                | Self::InvalidQuery { .. }
                | Self::InvalidBody(_)
                | Self::Unauthorized
                | Self::Forbidden
        )
    }

    /// Adds context to the error.
    pub fn with_context(self, context: impl Into<String>) -> Self {
        match self {
            Self::Internal(msg) => Self::Internal(format!("{}: {}", context.into(), msg)),
            Self::NotFound(msg) => Self::NotFound(format!("{}: {}", context.into(), msg)),
            Self::Unavailable(msg) => Self::Unavailable(format!("{}: {}", context.into(), msg)),
            other => other,
        }
    }
}

/// Result type alias for API operations.
pub type Result<T> = std::result::Result<T, ApiError>;

/// Convert ApiError to Axum response.
impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status_code();
        let body = crate::api::types::ApiResponse::<()>::error_message(self.to_string());

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_api_error_display() {
        let err = ApiError::NotFound("asteroid".to_string());
        assert_eq!(format!("{}", err), "resource not found: asteroid");

        let err = ApiError::InvalidQuery {
            param: "page".to_string(),
            reason: "must be positive".to_string(),
        };
        assert_eq!(
            format!("{}", err),
            "invalid query parameter 'page': must be positive"
        );
    }

    #[test]
    fn test_status_code() {
        assert_eq!(
            ApiError::NotFound("x".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::InvalidQuery {
                param: "x".to_string(),
                reason: "y".to_string()
            }
            .status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ApiError::Unauthorized.status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            ApiError::Internal("err".to_string()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_is_retryable() {
        let retryable = ApiError::Timeout {
            timeout: std::time::Duration::from_secs(30),
        };
        assert!(retryable.is_retryable());

        let non_retryable = ApiError::NotFound("x".to_string());
        assert!(!non_retryable.is_retryable());
    }

    #[test]
    fn test_is_client_error() {
        let client_err = ApiError::InvalidBody("bad json".to_string());
        assert!(client_err.is_client_error());

        let server_err = ApiError::Internal("db error".to_string());
        assert!(!server_err.is_client_error());
    }
}
