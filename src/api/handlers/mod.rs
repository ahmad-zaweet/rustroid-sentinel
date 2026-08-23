//! # API Handlers Module
//!
//! This module contains request handlers for each API endpoint.

pub mod approaches;
pub mod catalog;
pub mod dashboard;
pub mod etl_runs;
pub mod hazard_events_sse;
pub mod health;
pub mod internal_events;
pub mod stats;
pub mod velocity;

// Re-export handlers for backward compatibility
pub use approaches::approaches;
pub use catalog::{
    catalog_detail, catalog_list, catalog_rows, catalog_similar, render_catalog_detail_page,
    render_catalog_page,
};
pub use dashboard::{
    dashboard_table, format_number, refresh_metrics, refresh_velocity_chart, refresh_weekly_report,
    render_dashboard,
};
pub use etl_runs::{dashboard_etl_runs, etl_runs};
pub use hazard_events_sse::hazard_events_stream;
pub use health::health;
pub use internal_events::ingest_events;
pub use stats::stats;
pub use velocity::velocity;

// Re-export query types
pub use approaches::ApproachesQuery;
pub use dashboard::DashboardFilters;
pub use etl_runs::EtlRunsQuery;
pub use velocity::VelocityQuery;
