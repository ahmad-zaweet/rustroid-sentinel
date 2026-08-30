//! # Server-Side Templates
//! Internal module - documentation omitted for template structs.
#![allow(missing_docs)]
//!
//! This module contains Askama templates for server-side rendered (SSR) HTML pages.

use crate::api::types::{
    ApproachRecord, AsteroidCatalogRecord, AsteroidDetailRecord, EtlRunRecord, VelocityDataPoint,
};
use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

/// Application version, from `CARGO_PKG_VERSION` — rendered in the footer
/// of every full-page (non-partial) template.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Content hash of `static/css/dist.css`, computed once at startup.
///
/// `static_cache_control` sends `public, max-age=3600` for everything under
/// `/static`, and `dist.css` is referenced by a fixed filename — without
/// this, a CSS deploy would leave browsers serving the stale stylesheet for
/// up to an hour against newly-changed markup. Appending the hash as a `?v=`
/// query string on the `<link>` in `base.html` means a content change always
/// produces a new URL, so the long `max-age` is safe. Falls back to
/// `APP_VERSION` if the file can't be read (e.g. before the first
/// `npm run build:css`).
static CSS_VERSION: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| {
    std::fs::read("static/css/dist.css").map_or_else(
        |_| APP_VERSION.to_string(),
        |bytes| {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            bytes.hash(&mut hasher);
            format!("{:x}", hasher.finish())
        },
    )
});

/// Returns the cache-busting version string for `/css/dist.css`. See
/// [`CSS_VERSION`].
pub fn css_version() -> &'static str {
    &CSS_VERSION
}

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
    /// App version, rendered in the footer.
    pub version: &'static str,
    /// Current year, rendered in the footer's copyright line.
    pub current_year: i32,
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
    pub sentry_only: bool,
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
    pub cache_hit_rate_percent: String,
    pub cache_hit_rate_class: String,
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

/// Askama template for the weekly report partial (HTMX). Mirrors the
/// trailing-7-day summary sent to Discord by `rustroid-sentinel report`.
#[derive(Template)]
#[template(path = "partials/weekly-report.html")]
pub struct WeeklyReportTemplate {
    pub start_date: String,
    pub end_date: String,
    pub total_approaches: String,
    pub critical_count: String,
    pub high_count: String,
    pub medium_count: String,
    pub low_count: String,
    pub closest_approach: Option<String>,
    pub fastest_approach: Option<String>,
    pub largest_asteroid: Option<String>,
}

impl IntoResponse for WeeklyReportTemplate {
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

/// Askama template for the asteroid catalog page.
#[derive(Template)]
#[template(path = "dashboard/catalog.html")]
pub struct CatalogTemplate {
    /// Pre-rendered [`CatalogRowsTemplate`] HTML for the first page,
    /// injected with `| safe` — keeps a single source of truth for row
    /// markup between the full-page load and the HTMX rows endpoint.
    pub rows_html: String,
    /// Opaque cursor for the page after the first, or `None` if the first
    /// page is also the last — seeds the Next button's initial state.
    pub next_cursor: Option<String>,
    /// `cursor_history` value the Next button should send — the current
    /// page's cursor stack plus this page's own cursor token.
    pub next_history: String,
    /// Whether a Prev page exists (false on the first page).
    pub has_prev: bool,
    /// Cursor the Prev button should request, or `""` if the target is the
    /// cursor-less first page.
    pub prev_cursor: String,
    /// `cursor_history` value the Prev button should send — the current
    /// page's cursor stack with its last entry popped.
    pub prev_history: String,
    /// Active sort key, serialized form (e.g. `"name"`) — used to render
    /// the initial active-sort header styling.
    pub sort: String,
    /// Active sort direction, `"asc"` or `"desc"`.
    pub sort_dir: String,
    /// Current filter values as a URL query fragment (leading `&`, or
    /// empty), so sort-header links preserve the active filters.
    pub query_string: String,
    /// Distinct `orbit_class` values currently present in `asteroid_orbits`,
    /// sorted alphabetically — populates the orbit-class filter dropdown.
    pub orbit_classes: Vec<String>,
    /// Distinct `spectral_class` values currently present in
    /// `asteroid_orbits`, sorted alphabetically — populates the
    /// spectral-class filter dropdown.
    pub spectral_classes: Vec<String>,
    /// App version, rendered in the footer.
    pub version: &'static str,
    /// Current year, rendered in the footer's copyright line.
    pub current_year: i32,
}

impl IntoResponse for CatalogTemplate {
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

/// Askama template for one page of catalog rows, rendered behind explicit
/// Prev/Next buttons (see `templates/dashboard/catalog.html`'s inline
/// script) rather than infinite scroll.
///
/// Serves two roles: rendered directly as the `GET /dashboard/catalog/rows`
/// response, and pre-rendered once into [`CatalogTemplate::rows_html`] for
/// the initial page load.
#[derive(Template)]
#[template(path = "partials/catalog-rows.html")]
pub struct CatalogRowsTemplate {
    pub rows: Vec<AsteroidCatalogRecord>,
    /// Opaque cursor for the next page, or `None` if this was the last page.
    /// Emitted as an out-of-band swap so the pagination footer's Next
    /// button stays in sync with whatever page is currently displayed.
    pub next_cursor: Option<String>,
    /// `cursor_history` value the Next button should send.
    pub next_history: String,
    /// Whether a Prev page exists (false on the first page).
    pub has_prev: bool,
    /// Cursor the Prev button should request, or `""` if the target is the
    /// cursor-less first page.
    pub prev_cursor: String,
    /// `cursor_history` value the Prev button should send.
    pub prev_history: String,
    /// Whether to render the out-of-band Next-button and sort-header sync
    /// blocks. `true` for the live `/dashboard/catalog/rows` response;
    /// `false` when pre-rendering rows to embed in [`CatalogTemplate`],
    /// which renders its own Next button and header row directly (embedded
    /// OOB copies would collide on `id`).
    pub oob_next_button: bool,
    /// Active sort key, serialized form (e.g. `"name"`) — used to render
    /// the out-of-band sort-header active/toggle state.
    pub sort: String,
    /// Active sort direction, `"asc"` or `"desc"`.
    pub sort_dir: String,
    /// Current filter values as a URL query fragment (leading `&`, or
    /// empty), so sort-header links preserve the active filters.
    pub query_string: String,
}

impl IntoResponse for CatalogRowsTemplate {
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

/// Askama template for the asteroid detail page.
#[derive(Template)]
#[template(path = "dashboard/catalog-detail.html")]
pub struct CatalogDetailTemplate {
    pub asteroid: AsteroidDetailRecord,
    /// Nearest neighbors by pgvector embedding distance (M5). Empty if the
    /// asteroid hasn't been vectorized yet — an operational state, not an
    /// error, so the template renders nothing rather than an empty-state card.
    pub similar: Vec<AsteroidCatalogRecord>,
    /// App version, rendered in the footer.
    pub version: &'static str,
    /// Current year, rendered in the footer's copyright line.
    pub current_year: i32,
}

impl IntoResponse for CatalogDetailTemplate {
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
