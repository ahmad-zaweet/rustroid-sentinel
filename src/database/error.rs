//! # Database Error Types
//!
//! This module defines error types for database operations.
//! It provides structured errors for query execution, connection management,
//! and data access operations.

use thiserror::Error;

/// Errors that can occur during database operations.
#[derive(Error, Debug)]
pub enum DatabaseError {
    /// Failed to execute a database query.
    #[error("query execution failed: {0}")]
    QueryFailed(#[from] sqlx::Error),

    /// Database connection pool exhausted or unavailable.
    #[error("connection pool error: {0}")]
    ConnectionPool(String),

    /// Lost database connection during operation.
    #[error("connection lost: {0}")]
    ConnectionLost(String),

    /// Failed to acquire a connection from the pool within timeout.
    #[error("connection acquisition timeout after {timeout:?}")]
    ConnectionTimeout {
        /// The timeout duration that was exceeded.
        timeout: std::time::Duration,
    },

    /// Database migration failed.
    #[error("migration failed: {0}")]
    MigrationFailed(String),

    /// No rows returned when at least one was expected.
    #[error("no rows returned")]
    NoRowsReturned,

    /// Multiple rows returned when only one was expected.
    #[error("multiple rows returned when only one was expected")]
    TooManyRows,

    /// Data validation error (e.g., constraint violation).
    #[error("data validation error: {0}")]
    ValidationError(String),

    /// Database operation timed out.
    #[error("database operation timed out after {timeout:?}")]
    Timeout {
        /// The timeout duration that was exceeded.
        timeout: std::time::Duration,
    },
}

impl DatabaseError {
    /// Returns true if the error is retryable.
    ///
    /// Connection errors and timeouts are considered retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::ConnectionLost(_)
            | Self::ConnectionPool(_)
            | Self::ConnectionTimeout { .. }
            | Self::Timeout { .. } => true,
            Self::QueryFailed(sqlx::Error::Io(e)) => {
                matches!(
                    e.kind(),
                    std::io::ErrorKind::ConnectionRefused
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::NotConnected
                        | std::io::ErrorKind::TimedOut
                )
            }
            _ => false,
        }
    }

    /// Returns true if the error is a connection-related error.
    pub fn is_connection_error(&self) -> bool {
        matches!(
            self,
            Self::ConnectionPool(_) | Self::ConnectionLost(_) | Self::ConnectionTimeout { .. }
        )
    }

    /// Returns true if the error indicates a constraint violation.
    pub fn is_constraint_violation(&self) -> bool {
        matches!(self, Self::ValidationError(_))
    }
}

/// Result type alias for database operations.
pub type Result<T> = std::result::Result<T, DatabaseError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_error_display() {
        let err = DatabaseError::NoRowsReturned;
        assert_eq!(format!("{}", err), "no rows returned");

        let err = DatabaseError::ConnectionTimeout {
            timeout: std::time::Duration::from_secs(30),
        };
        assert_eq!(
            format!("{}", err),
            "connection acquisition timeout after 30s"
        );
    }

    #[test]
    fn test_is_retryable() {
        let retryable = DatabaseError::ConnectionLost("network error".to_string());
        assert!(retryable.is_retryable());

        let non_retryable = DatabaseError::NoRowsReturned;
        assert!(!non_retryable.is_retryable());
    }

    #[test]
    fn test_is_connection_error() {
        let conn_err = DatabaseError::ConnectionPool("exhausted".to_string());
        assert!(conn_err.is_connection_error());

        let query_err = DatabaseError::NoRowsReturned;
        assert!(!query_err.is_connection_error());
    }
}
