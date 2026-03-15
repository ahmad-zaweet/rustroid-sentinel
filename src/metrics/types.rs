//! # Metrics Type Definitions
//!
//! This module contains the core data structures for metrics collection and reporting.

/// A summary of key system and business metrics.
///
/// This struct consolidates data from both the metrics system (performance)
/// and the database (domain counts) to provide a unified overview for the dashboard.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricsSummary {
    /// Average HTTP requests per second over the last minute.
    pub requests_per_second: f64,
    /// Percentage of requests that resulted in 4xx or 5xx errors.
    pub error_rate_percent: f64,
    /// Average response time in milliseconds.
    pub avg_response_time_ms: f64,
    /// Average database queries per second.
    pub db_queries_per_second: f64,
    /// Total number of unique asteroids in the system.
    pub total_asteroids: i64,
    /// Total number of recorded close approach events.
    pub total_approaches: i64,
    /// Number of asteroids flagged as potentially hazardous.
    pub hazardous_count: i64,
    /// UNIX timestamp of the last successful ETL run completion.
    pub last_etl_run: Option<i64>,
}

impl Default for MetricsSummary {
    fn default() -> Self {
        Self {
            requests_per_second: 0.0,
            error_rate_percent: 0.0,
            avg_response_time_ms: 0.0,
            db_queries_per_second: 0.0,
            total_asteroids: 0,
            total_approaches: 0,
            hazardous_count: 0,
            last_etl_run: None,
        }
    }
}

/// Internal struct for collecting database-specific metrics.
#[derive(Debug, Default, Clone)]
pub struct DatabaseMetrics {
    /// Total number of unique asteroids in the database.
    pub total_asteroids: i64,
    /// Total number of recorded close approach events.
    pub total_approaches: i64,
    /// Number of asteroids flagged as potentially hazardous.
    pub hazardous_count: i64,
    /// UNIX timestamp of the last successful ETL run.
    pub last_etl_run: Option<i64>,
}
