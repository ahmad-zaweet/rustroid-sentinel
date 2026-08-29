//! # API Types
//!
//! This module defines shared types used across the API layer for requests, responses,
//! and data transfer objects (DTOs). All types implement [`serde::Serialize`] and
//! [`serde::Deserialize`] for JSON serialization.

use std::path::Path;

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
    /// NASA's unique identifier for the parent asteroid.
    pub neo_reference_id: String,
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
    /// Average (midpoint) estimated diameter in kilometers.
    pub estimated_diameter_avg_km: f64,
    /// JPL Sentry Torino Scale (0-10), if this asteroid is a currently
    /// tracked virtual impactor. `None` for the vast majority of asteroids.
    pub torino_scale: Option<i16>,
    /// JPL Sentry cumulative Palermo Scale, if this asteroid is a currently
    /// tracked virtual impactor.
    pub palermo_scale: Option<f64>,
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

impl EtlRunRecord {
    /// Renders [`Self::source_file`] as a human-friendly label.
    ///
    /// Source files follow the pattern `{prefix}_{YYYYMMDD}_{YYYYMMDD}_{suffix}.json`
    /// (e.g. `asteroids_20260828_20260829_transformed.json` → `"Asteroids · Aug 28 – Aug 29, 2026"`).
    /// Falls back to the extension-stripped filename if it doesn't match that shape.
    pub fn display_name(&self) -> String {
        let stem = Path::new(&self.source_file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&self.source_file);

        let mut parts = stem.splitn(4, '_');
        let (Some(prefix), Some(start), Some(end)) = (parts.next(), parts.next(), parts.next())
        else {
            return stem.to_string();
        };
        let Ok(start_date) = NaiveDate::parse_from_str(start, "%Y%m%d") else {
            return stem.to_string();
        };
        let Ok(end_date) = NaiveDate::parse_from_str(end, "%Y%m%d") else {
            return stem.to_string();
        };

        let mut label = prefix.to_string();
        if let Some(c) = label.get_mut(0..1) {
            c.make_ascii_uppercase();
        }

        let range = if start_date == end_date {
            start_date.format("%b %e, %Y").to_string()
        } else {
            format!(
                "{} – {}",
                start_date.format("%b %e"),
                end_date.format("%b %e, %Y")
            )
        };

        format!("{label} · {range}")
    }
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

/// Sort key for the asteroid catalog listing (`GET /asteroids`).
///
/// Deserializes into a fixed enum rather than accepting a raw column name,
/// so a client-supplied sort parameter can never reach SQL as a string.
/// [`crate::database::catalog::CatalogRepository`] matches each variant to a
/// fixed `ORDER BY` expression.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CatalogSortKey {
    /// By each asteroid's most recent recorded close-approach date (`NULL` —
    /// no recorded approaches — always sorts last regardless of direction).
    #[default]
    #[serde(rename = "approach_activity")]
    ApproachActivity,
    /// By asteroid name.
    #[serde(rename = "name")]
    Name,
    /// By average estimated diameter, in kilometers.
    #[serde(rename = "diameter")]
    Diameter,
    /// By Torino Scale (`NULL` sorts last regardless of direction).
    #[serde(rename = "torino")]
    Torino,
    /// By cumulative Palermo Scale (`NULL` sorts last regardless of direction).
    #[serde(rename = "palermo")]
    Palermo,
}

/// Sort direction for the asteroid catalog listing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortDir {
    /// Ascending order for the sort direction for the asteroid catalog.
    Asc,
    /// Descending order for the sort direction for the asteroid catalog.
    #[default]
    Desc,
}

/// Query parameters for the asteroid catalog listing (`GET /asteroids`).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CatalogQuery {
    /// Opaque keyset cursor from a previous page's `next_cursor`. Omitted
    /// (or invalid) starts from the first page.
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub cursor: Option<String>,
    /// Comma-separated stack of cursor tokens for every page visited before
    /// this one (oldest first; an empty entry represents the cursor-less
    /// first page), so the Prev link can pop one off without server-side
    /// session state — see `build_catalog_query_string`/pagination link
    /// rendering in `src/api/handlers/catalog.rs`.
    pub cursor_history: Option<String>,
    /// Sort key; only one exists today, but the query shape is fixed now so
    /// adding one later doesn't change the DTO.
    #[serde(default)]
    pub sort: CatalogSortKey,
    /// Sort direction.
    #[serde(default)]
    pub sort_dir: SortDir,
    /// Case-insensitive substring match against `asteroids.name`, backed by
    /// the `pg_trgm` GIN index.
    pub name: Option<String>,
    /// Filter to potentially-hazardous asteroids only.
    pub is_potentially_hazardous: Option<bool>,
    /// Filter to currently-flagged JPL Sentry objects only.
    pub is_sentry_object: Option<bool>,
    /// Earliest close-approach date to include (inclusive).
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub start_date: Option<NaiveDate>,
    /// Latest close-approach date to include (inclusive).
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub end_date: Option<NaiveDate>,
    /// Minimum average estimated diameter, in kilometers (inclusive).
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub min_diameter_km: Option<f64>,
    /// Maximum average estimated diameter, in kilometers (inclusive).
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub max_diameter_km: Option<f64>,
    /// Orbit classification filter (e.g. "Aten"), from `asteroid_orbits`.
    pub orbit_class: Option<String>,
    /// Spectral classification filter (e.g. "Sq"), from `asteroid_orbits`.
    pub spectral_class: Option<String>,
    /// Minimum Torino Scale (0-10, inclusive).
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub min_torino_scale: Option<i16>,
    /// Minimum cumulative Palermo Scale (inclusive).
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub min_palermo_scale: Option<f64>,
}

/// Deserializes an empty query-string value (`key=`) as `None` instead of a
/// parse error. HTML form inputs (dates, numbers) submit `""` when left
/// blank, and `hx-params` has no built-in way to strip those before they
/// hit the wire, so the query DTO has to tolerate them directly.
fn empty_string_as_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => s.parse::<T>().map(Some).map_err(serde::de::Error::custom),
    }
}

/// A single row in the asteroid catalog listing.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AsteroidCatalogRecord {
    /// Internal identifier.
    pub id: Uuid,
    /// NASA's unique identifier for this asteroid.
    pub neo_reference_id: String,
    /// Human-readable name or designation.
    pub name: String,
    /// Average (midpoint) estimated diameter in kilometers.
    pub estimated_diameter_avg_km: f64,
    /// Whether NASA designates this as a Potentially Hazardous Asteroid.
    pub is_potentially_hazardous: bool,
    /// Whether this asteroid is currently on JPL's Sentry Risk List.
    pub is_sentry_object: bool,
    /// JPL Sentry Torino Scale (0-10), if a currently tracked virtual impactor.
    pub torino_scale: Option<i16>,
    /// JPL Sentry cumulative Palermo Scale, if a currently tracked virtual impactor.
    pub palermo_scale: Option<f64>,
    /// Dynamical orbit classification (e.g. "Aten"), from JPL's SBDB.
    pub orbit_class: Option<String>,
    /// Spectral classification (e.g. "Sq"), from JPL's SBDB.
    pub spectral_class: Option<String>,
    /// `MAX(close_approach_date)` across this asteroid's recorded
    /// approaches; `None` if it has none. This is the catalog's sort key.
    pub latest_approach_date: Option<NaiveDate>,
}

/// Detail view for a single asteroid (`GET /asteroids/{neo_reference_id}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsteroidDetailRecord {
    /// The catalog-summary fields, shared with the listing row.
    #[serde(flatten)]
    pub summary: AsteroidCatalogRecord,
    /// Orbital eccentricity (dimensionless), from JPL's SBDB.
    pub eccentricity: Option<f64>,
    /// Semi-major axis, in AU, from JPL's SBDB.
    pub semi_major_axis_au: Option<f64>,
    /// Inclination to the ecliptic, in degrees, from JPL's SBDB.
    pub inclination_deg: Option<f64>,
    /// Geometric albedo (dimensionless), from JPL's SBDB.
    pub albedo: Option<f64>,
    /// Link to NASA's JPL Small-Body Database page.
    pub nasa_jpl_url: String,
    /// This asteroid's recorded close-approach history, most recent first.
    pub approaches: Vec<ApproachRecord>,
}

/// A keyset-paginated page of results.
///
/// Unlike [`PaginatedResponse`], this carries no total count or page
/// number — keyset pagination only ever moves forward, one opaque cursor
/// at a time, so there's nothing else to expose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPage<T> {
    /// The page of data items.
    pub data: Vec<T>,
    /// Opaque cursor for the next page, or `None` if this was the last page.
    pub next_cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test: HTML date/number inputs submit `""` when left
    /// blank, and htmx's `hx-params` has no way to strip those before the
    /// request goes out. If the filter form is only partially filled in
    /// (the common case), every untouched date/number field arrives as an
    /// empty string alongside the ones the user actually set — this must
    /// not 400.
    #[test]
    fn catalog_query_treats_empty_values_as_none() {
        let query: CatalogQuery = serde_urlencoded::from_str(
            "name=apophis&start_date=&end_date=&min_diameter_km=&max_diameter_km=&min_torino_scale=&min_palermo_scale=",
        )
        .expect("empty optional fields should deserialize, not error");

        assert_eq!(query.name.as_deref(), Some("apophis"));
        assert_eq!(query.start_date, None);
        assert_eq!(query.end_date, None);
        assert_eq!(query.min_diameter_km, None);
        assert_eq!(query.max_diameter_km, None);
        assert_eq!(query.min_torino_scale, None);
        assert_eq!(query.min_palermo_scale, None);
    }

    #[test]
    fn catalog_query_still_parses_populated_values() {
        let query: CatalogQuery =
            serde_urlencoded::from_str("start_date=2026-01-01&min_torino_scale=3")
                .expect("populated fields should deserialize");

        assert_eq!(
            query.start_date,
            Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
        );
        assert_eq!(query.min_torino_scale, Some(3));
    }

    #[test]
    fn catalog_query_rejects_genuinely_malformed_values() {
        let result: Result<CatalogQuery, _> = serde_urlencoded::from_str("start_date=not-a-date");
        assert!(result.is_err());
    }
}
