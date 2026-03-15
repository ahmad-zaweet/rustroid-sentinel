//! # Alert Error Types
//!
//! This module defines error types for alerting operations.
//! It provides structured errors for Discord notifications, alert service operations,
//! and alert persistence.

use thiserror::Error;

/// Errors that can occur during alert operations.
#[derive(Error, Debug)]
pub enum AlertError {
    /// Failed to send Discord notification.
    #[error("failed to send Discord alert: {0}")]
    DiscordNotification(#[from] serenity::Error),

    /// Discord webhook URL is missing or invalid.
    #[error("Discord webhook URL is not configured")]
    DiscordWebhookNotConfigured,

    /// Failed to serialize alert payload.
    #[error("failed to serialize alert payload: {0}")]
    PayloadSerialization(#[from] serde_json::Error),

    /// Failed to record alert in database or query for unalerted approaches.
    #[error("database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    /// Alert service not properly initialized.
    #[error("alert service not initialized: {0}")]
    NotInitialized(String),

    /// Failed to spawn alert task.
    #[error("failed to spawn alert task: {0}")]
    TaskSpawnFailed(String),

    /// Alert task panicked or failed to join.
    #[error("alert task failed: {0}")]
    TaskFailed(String),

    /// Rate limit exceeded for alert channel.
    #[error("rate limit exceeded; retry after {retry_after:?}")]
    RateLimited {
        /// Duration to wait before retrying.
        retry_after: std::time::Duration,
    },

    /// External notification service unavailable.
    #[error("notification service unavailable: {0}")]
    ServiceUnavailable(String),
}

impl AlertError {
    /// Returns true if the error is retryable.
    ///
    /// Network errors, rate limits, and service unavailability are considered retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::DiscordNotification(e) => {
                // Serenity errors don't have structured retry info,
                // so we check for common retryable conditions
                matches!(e, serenity::Error::Http(_))
            }
            Self::RateLimited { .. } | Self::ServiceUnavailable(_) | Self::TaskFailed(_) => true,
            _ => false,
        }
    }

    /// Returns true if the error is a configuration error.
    pub fn is_config_error(&self) -> bool {
        matches!(
            self,
            Self::DiscordWebhookNotConfigured | Self::NotInitialized(_)
        )
    }

    /// Returns the retry-after duration if rate limited.
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            Self::RateLimited { retry_after } => Some(*retry_after),
            _ => None,
        }
    }
}

/// Result type alias for alert operations.
pub type Result<T> = std::result::Result<T, AlertError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_error_display() {
        let err = AlertError::DiscordWebhookNotConfigured;
        assert_eq!(format!("{}", err), "Discord webhook URL is not configured");

        let err = AlertError::RateLimited {
            retry_after: std::time::Duration::from_secs(60),
        };
        assert_eq!(format!("{}", err), "rate limit exceeded; retry after 60s");
    }

    #[test]
    fn test_is_retryable() {
        let retryable = AlertError::ServiceUnavailable("Discord API down".to_string());
        assert!(retryable.is_retryable());

        let non_retryable = AlertError::DiscordWebhookNotConfigured;
        assert!(!non_retryable.is_retryable());
    }

    #[test]
    fn test_is_config_error() {
        let config_err = AlertError::DiscordWebhookNotConfigured;
        assert!(config_err.is_config_error());

        let network_err = AlertError::TaskFailed("timeout".to_string());
        assert!(!network_err.is_config_error());
    }

    #[test]
    fn test_retry_after() {
        let rate_limit = AlertError::RateLimited {
            retry_after: std::time::Duration::from_secs(120),
        };
        assert_eq!(
            rate_limit.retry_after(),
            Some(std::time::Duration::from_secs(120))
        );

        let other = AlertError::TaskFailed("error".to_string());
        assert_eq!(other.retry_after(), None);
    }
}
