//! # API Handlers Module
//!
//! This module contains request handlers for each API endpoint.

pub mod approaches;
pub mod dashboard;
pub mod etl_runs;
pub mod health;
pub mod stats;
pub mod velocity;

// Re-export handlers for backward compatibility
pub use approaches::approaches;
pub use dashboard::{dashboard_table, format_number, render_dashboard};
pub use etl_runs::{dashboard_etl_runs, etl_runs};
pub use health::health;
pub use stats::stats;
pub use velocity::velocity;

// Re-export query types
pub use approaches::ApproachesQuery;
pub use dashboard::DashboardFilters;
pub use etl_runs::EtlRunsQuery;
pub use velocity::VelocityQuery;
