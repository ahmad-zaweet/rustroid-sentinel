//! # Error Types
//!
//! This module defines the error types used throughout the Rustroid Sentinel application.
//! It provides a unified error handling mechanism using `thiserror` for structured errors.
//!
//! ## Error Hierarchy
//!
//! ### Application-Level Errors
//!
//! - `Error` - The primary error type for the application
//!   - `Config` - Configuration loading/parsing failures
//!   - `Http` - HTTP client errors
//!   - `Database` - SQLx database operation failures
//!   - `Transform` - Data transformation errors
//!
//! ### Module-Specific Error Types
//!
//! Each module exports its own error types for fine-grained error handling:
//!
//! - [`crate::api::error::ApiError`] - API request/response errors
//! - [`crate::alert::error::AlertError`] - Alert notification errors
//! - [`crate::database::error::DatabaseError`] - Database operation errors
//! - [`crate::metrics::error::MetricsError`] - Metrics collection errors
//! - [`crate::nasa::error::NasaApiError`] - NASA API integration errors
//!
//! ### Configuration Errors
//!
//! - `ServiceConfigError` - Specific errors during configuration loading
//!   - `MissingFile` - Required configuration file not found
//!   - `FileParse` - TOML syntax error
//!   - `Deserialize` - Type mismatch or missing required field
//!   - `Unexpected` - Unknown config error
//!
//! - `HttpClientError` - HTTP client construction errors
//!
//! ## Error Handling Strategy
//!
//! | Scenario | Recommended Pattern |
//! |----------|-------------------|
//! | **Application logic** (CLI, API handlers, ETL) | `anyhow::Result<T>` + `.context()` |
//! | **Public library API** (exported types) | Module-specific `thiserror` enum |
//! | **Internal helpers** | Propagate upstream errors |
//! | **Domain validation** | Custom `thiserror` with structured fields |
//!
//! ## Example
//!
//! ```rust
//! use rustroid_sentinel::error::{Error, ServiceConfigError};
//!
//! fn load_config() -> Result<(), Error> {
//!     // Configuration loading that might fail
//!     Ok(())
//! }
//! ```

// Re-export module error types for convenient access
pub use crate::alert::error::AlertError;
pub use crate::api::error::ApiError;
pub use crate::database::error::DatabaseError;
pub use crate::metrics::error::MetricsError;

use thiserror::Error;

/// The primary error type for the Rustroid Sentinel application.
///
/// This enum encapsulates all possible errors that can occur within the
/// application, providing a unified error handling mechanism.
#[derive(Debug, Error)]
pub enum Error {
    /// Represents an error related to configuration loading, parsing, or validation.
    ///
    /// This variant wraps the `ServiceConfigError` enum, which provides more specific details
    /// about the nature of the configuration failure.
    #[error("Configuration error: {0}")]
    Config(#[from] ServiceConfigError),

    /// Represents an error occurring within a shared HTTP client.
    #[error("Http client error: {0}")]
    Http(#[from] HttpClientError),

    /// Represents a failure in database operations (via `sqlx`).
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Represents a failure during the data transformation or classification process.
    #[error("Transform error: {0}")]
    Transform(String),
}

/// Represents specific errors that can occur during the configuration process.
///
/// This enum is designed to provide detailed, user-friendly feedback when
/// the application fails to load its configuration correctly.
#[derive(Debug, Error)]
pub enum ServiceConfigError {
    /// A required configuration file could not be found.
    #[error("A required configuration file was not found: {0}")]
    MissingFile(String),

    /// A configuration file was found but could not be parsed.
    /// This often indicates a syntax error (e.g., invalid TOML).
    #[error("Failed to parse configuration file '{0}'")]
    FileParse(String),

    /// The configuration was successfully loaded and parsed, but it could not be
    /// deserialized into the `RustroidSentinelConfig` struct. This is typically
    /// caused by a type mismatch or a missing required field.
    #[error("Failed to deserialize configuration: {0}")]
    Deserialize(String),

    /// The configuration file loading or parsing failed due to an unexpected error.
    #[error("An unknown error occurred when loading/parsing configuration file: {0}")]
    Unexpected(String),
}

/// Errors related to shared HTTP client operations.
#[derive(Debug, Error)]
pub enum HttpClientError {
    /// Failed to build the shared HTTP client.
    #[error("An error occurred when building the shared http client: {0}")]
    HttpClientBuild(String),

    /// Errors from the underlying reqwest client.
    #[error("HTTP request error: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// Errors from the HTTP middleware.
    #[error("HTTP middleware error: {0}")]
    Middleware(#[from] reqwest_middleware::Error),
}
