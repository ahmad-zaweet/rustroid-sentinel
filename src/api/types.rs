//! # API Types
//!
//! This module defines shared types used across the API layer for requests, responses,
//! and data transfer objects (DTOs). All types implement [`serde::Serialize`] and
//! [`serde::Deserialize`] for JSON serialization.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Standardized API response wrapper for consistent JSON responses.
///
/// This generic struct ensures that all API endpoints return a predictable
/// JSON structure, regardless of whether the request succeeded or failed.
///
/// # Type Parameters
///
/// * `T` - The type of the data payload for successful responses.
///
/// # Examples
///
/// ## Successful Response
/// ```json
/// {
///   "success": true,
///   "data": { "status": "OK" }
/// }
/// ```
///
/// ## Error Response
/// ```json
/// {
///   "success": false,
///   "error": "Database connection failed"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Indicates if the request was processed successfully.
    pub success: bool,
    /// The payload of the response, omitted if the request failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// A human-readable error message, omitted if the request succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    /// Creates a new successful API response with the given data.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rustroid_sentinel::api::types::ApiResponse;
    ///
    /// let response = ApiResponse::success("Operation completed");
    /// assert!(response.success);
    /// assert_eq!(response.data, Some("Operation completed"));
    /// ```
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// Creates a new error API response with the given message.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rustroid_sentinel::api::types::ApiResponse;
    ///
    /// let response: ApiResponse<String> = ApiResponse::error_message("Something went wrong".to_string());
    /// assert!(!response.success);
    /// assert_eq!(response.error, Some("Something went wrong".to_string()));
    /// ```
    pub fn error_message(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// Health check response.
///
/// Provides basic status information about the service and its dependencies.
/// Used by load balancers and monitoring systems to verify service availability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// The general status of the service (e.g., "healthy", "degraded").
    pub status: String,
    /// The current version of the application (from `CARGO_PKG_VERSION`).
    pub version: String,
    /// The server time when the health check was performed.
    pub timestamp: DateTime<Utc>,
    /// Whether the database is currently reachable.
    pub database_connected: bool,
}

/// Statistics about asteroids and approaches.
///
/// This response is used by the dashboard's primary data visualization components
/// to display high-level metrics and recent activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResponse {
    /// Total number of unique asteroids in the database.
    pub total_asteroids: i64,
    /// Total number of recorded close approach events.
    pub total_approaches: i64,
    /// Number of asteroids flagged as potentially hazardous.
    pub hazardous_count: i64,
    /// List of recent approach records for display.
    pub recent_approaches: Vec<ApproachRecord>,
    /// Time-series data for velocity visualization.
    pub velocity_data: Vec<VelocityDataPoint>,
}

/// A single approach record for display.
///
/// This struct represents a denormalized view of an approach event,
/// including the parent asteroid's name for display purposes.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApproachRecord {
    /// Unique identifier for the approach event.
    pub id: Uuid,
    /// Human-readable name of the asteroid.
    pub asteroid_name: String,
    /// The date of closest approach to Earth.
    pub close_approach_date: NaiveDate,
    /// Relative velocity in kilometers per hour.
    pub velocity_km_per_h: f64,
    /// Miss distance in kilometers.
    pub miss_distance_km: f64,
    /// Hazard classification level (e.g., "Critical", "High", "Medium", "Low").
    pub hazard_classification: String,
    /// Whether NASA designates this as a Potentially Hazardous Asteroid.
    pub is_potentially_hazardous: bool,
}

/// Data point for velocity time-series chart.
///
/// Each point represents a single approach event with its velocity
/// and timestamp, used for plotting approach speeds over time.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VelocityDataPoint {
    /// The date of the approach event.
    pub date: NaiveDate,
    /// Name of the asteroid for labeling.
    pub asteroid_name: String,
    /// Relative velocity in kilometers per hour.
    pub velocity_km_per_h: f64,
}

/// ETL run record for monitoring.
///
/// Tracks the execution history of ETL jobs, including timing,
/// processing statistics, and any error messages.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EtlRunRecord {
    /// Unique identifier for the ETL run.
    pub id: Uuid,
    /// Source file or data identifier that was processed.
    pub source_file: String,
    /// When the ETL job started execution.
    pub started_at: DateTime<Utc>,
    /// When the ETL job completed (if successful).
    pub completed_at: Option<DateTime<Utc>>,
    /// Final status of the job.
    pub status: crate::models::EtlStatus,
    /// Number of asteroids inserted or updated.
    pub asteroids_processed: i32,
    /// Number of approach events inserted.
    pub approaches_processed: i32,
    /// Error message if the job failed, `None` otherwise.
    pub error_message: Option<String>,
}

/// Response for ETL runs endpoint.
///
/// Wrapper for returning a list of recent ETL execution records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtlRunsResponse {
    /// List of ETL run records, ordered by start time descending.
    pub runs: Vec<EtlRunRecord>,
}

/// Paginated response wrapper.
///
/// Generic wrapper for returning paginated collections with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    /// The page of data items.
    pub data: Vec<T>,
    /// Pagination metadata for client-side navigation.
    pub pagination: PaginationInfo,
}

/// Pagination metadata.
///
/// Provides information needed for client-side pagination controls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    /// Current page number (1-indexed).
    pub page: u32,
    /// Number of items per page.
    pub page_size: u32,
    /// Total number of items across all pages.
    pub total_items: i64,
    /// Total number of pages available.
    pub total_pages: u32,
}

/// Predefined time periods for filtering analysis data.
///
/// This enum provides type-safe time range selection for API queries,
/// with automatic conversion to PostgreSQL interval strings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TimePeriod {
    /// Last 7 days.
    #[serde(rename = "7d")]
    Last7Days,
    /// Last 30 days.
    #[serde(rename = "30d")]
    Last30Days,
    /// Last 90 days (default for many views).
    #[default]
    #[serde(rename = "90d")]
    Last90Days,
    /// Last 365 days.
    #[serde(rename = "1y")]
    LastYear,
}

impl TimePeriod {
    /// Returns the PostgreSQL interval string corresponding to the time period.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rustroid_sentinel::api::types::TimePeriod;
    ///
    /// assert_eq!(TimePeriod::Last7Days.as_sql_interval(), "7 days");
    /// assert_eq!(TimePeriod::Last90Days.as_sql_interval(), "90 days");
    /// ```
    pub fn as_sql_interval(&self) -> &'static str {
        match self {
            Self::Last7Days => "7 days",
            Self::Last30Days => "30 days",
            Self::Last90Days => "90 days",
            Self::LastYear => "1 year",
        }
    }
}
