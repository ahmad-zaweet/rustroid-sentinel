//! # Server-Side Templates
//! Internal module - documentation omitted for template structs.
#![allow(missing_docs)]
//!
//! This module contains Askama templates for server-side rendered (SSR) HTML pages.

use crate::api::types::{ApproachRecord, EtlRunRecord, VelocityDataPoint};
use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

/// Askama template for the main dashboard index page.
#[derive(Template)]
#[template(path = "dashboard/index.html")]
pub struct DashboardTemplate {
    pub total_asteroids: String,
    pub total_approaches: String,
    pub hazardous_count: String,
    pub approaches: Vec<ApproachRecord>,
    pub showing_start: u32,
    pub showing_end: u32,
    pub total_items: u32,
    pub current_page: u32,
    pub total_pages: u32,
    pub page_size: u32,
    /// Velocity data for the chart as JSON string (for JavaScript).
    pub velocity_data_json: String,
    /// Velocity data for the chart as Vec (for SSR template iteration).
    pub velocity_data: Vec<VelocityDataPoint>,
    /// Last updated timestamp for the dashboard.
    pub last_updated: String,
    /// Selected time period for the velocity chart (for active button state).
    pub period: String,
}

impl IntoResponse for DashboardTemplate {
    fn into_response(self) -> Response {
        match self.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template: {}", err),
            )
                .into_response(),
        }
    }
}

/// A page number with a flag indicating if it's the current page.
pub struct PageItem {
    pub number: u32,
    pub is_current: bool,
}

/// Askama template for the approaches table partial (HTMX).
#[derive(Template)]
#[template(path = "partials/approaches-table.html")]
pub struct ApproachesTableTemplate {
    pub approaches: Vec<ApproachRecord>,
    pub showing_start: u32,
    pub showing_end: u32,
    pub total_items: u32,
    pub current_page: u32,
    pub total_pages: u32,
    pub page_size: u32,
    pub page_range: Vec<PageItem>,
    pub query_string: String,
    pub hazard_selected: String,
    pub start_date_value: String,
    pub end_date_value: String,
    pub is_first_page: bool,
    pub is_last_page: bool,
    pub sort_by: String,
    pub sort_dir: String,
}

impl IntoResponse for ApproachesTableTemplate {
    fn into_response(self) -> Response {
        match self.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template: {}", err),
            )
                .into_response(),
        }
    }
}

/// Askama template for the ETL runs partial (HTMX).
#[derive(Template)]
#[template(path = "partials/etl-runs.html")]
pub struct EtlRunsTemplate {
    pub etl_runs: Vec<EtlRunRecord>,
    pub current_page: u32,
    pub total_pages: u32,
    pub page_size: u32,
}

impl IntoResponse for EtlRunsTemplate {
    fn into_response(self) -> Response {
        match self.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template: {}", err),
            )
                .into_response(),
        }
    }
}

/// Askama template for the velocity chart partial (HTMX).
#[derive(Template)]
#[template(path = "partials/velocity-chart.html")]
pub struct VelocityChartTemplate {
    pub velocity_data: Vec<VelocityDataPoint>,
    pub period: String,
    pub last_updated: String,
}

impl IntoResponse for VelocityChartTemplate {
    fn into_response(self) -> Response {
        match self.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template: {}", err),
            )
                .into_response(),
        }
    }
}

/// Askama template for the metrics cards partial (HTMX).
#[derive(Template)]
#[template(path = "partials/metrics.html")]
pub struct MetricsTemplate {
    pub requests_per_second: String,
    pub error_rate_percent: String,
    pub error_rate_class: String,
    pub avg_response_time_ms: String,
    pub db_queries_per_second: String,
    pub storage_used_percent: String,
    pub storage_class: String,
}

impl IntoResponse for MetricsTemplate {
    fn into_response(self) -> Response {
        match self.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template: {}", err),
            )
                .into_response(),
        }
    }
}
