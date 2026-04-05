//! Dashboard handlers (SSR and HTMX partials).

use axum::response::IntoResponse;
use axum::{extract::Query, extract::State};
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::{error, info};

use crate::api::templates::{
    ApproachesTableTemplate, DashboardTemplate, MetricsTemplate, PageItem, VelocityChartTemplate,
};
use crate::api::types::TimePeriod;
use crate::database::dashboard::DashboardRepository;
use crate::metrics::get_metrics_summary;
use crate::server::AppState;

/// Deserialize empty strings as `None` for query parameters.
fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => s.parse().map(Some).map_err(serde::de::Error::custom),
    }
}

/// Query parameters for dashboard filters.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct DashboardFilters {
    /// Page number for pagination.
    pub page: Option<u32>,
    /// Hazard classification filter.
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub hazard_class: Option<String>,
    /// Start date filter.
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub start_date: Option<NaiveDate>,
    /// End date filter.
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub end_date: Option<NaiveDate>,
}

impl DashboardFilters {
    /// Build a query string from non-empty filter values.
    pub fn to_query_string(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref hazard) = self.hazard_class
            && !hazard.is_empty()
        {
            parts.push(format!("hazard_class={}", urlencoding::encode(hazard)));
        }
        if let Some(ref start) = self.start_date {
            parts.push(format!("start_date={}", start));
        }
        if let Some(ref end) = self.end_date {
            parts.push(format!("end_date={}", end));
        }

        if parts.is_empty() {
            String::new()
        } else {
            format!("&{}", parts.join("&"))
        }
    }
}

/// GET /
///
/// Renders the main dashboard SSR template.
#[allow(clippy::cognitive_complexity)]
pub async fn render_dashboard(State(state): State<AppState>) -> impl IntoResponse {
    info!("Dashboard SSR requested");

    // Fetch stats
    let stats = match DashboardRepository::get_stats(&state.db_pool).await {
        Ok(s) => s,
        Err(e) => {
            error!("Dashboard stats query failed: {}", e);
            Default::default()
        }
    };

    // Fetch initial page of approaches (Page 1, Size 20)
    let page = 1;
    let page_size = 20;

    let (approaches, total_items) = match DashboardRepository::get_paginated_approaches(
        &state.db_pool,
        page,
        page_size,
        None,
        None,
        None,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            error!("Dashboard approaches query failed: {}", e);
            (Vec::new(), 0)
        }
    };

    // Fetch velocity data for the chart (last 7 days by default to match UI)
    let velocity_data = match DashboardRepository::get_velocity_data_by_period(
        &state.db_pool,
        crate::api::types::TimePeriod::Last7Days,
    )
    .await
    {
        Ok(data) => data,
        Err(e) => {
            error!("Dashboard velocity data query failed: {}", e);
            Vec::new()
        }
    };

    let total_pages = if total_items > 0 {
        ((total_items as f64) / (page_size as f64)).ceil() as u32
    } else {
        1
    };

    let showing_start = if total_items == 0 { 0 } else { 1 };
    let showing_end = std::cmp::min(page_size as i64, total_items);

    let now = chrono::Local::now();
    let last_updated = now.format("%I:%M:%S %p").to_string();

    DashboardTemplate {
        total_asteroids: format_number(stats.total_asteroids as f64),
        total_approaches: format_number(stats.total_approaches as f64),
        hazardous_count: format_number(stats.hazardous_count as f64),
        approaches,
        showing_start: showing_start as u32,
        showing_end: showing_end as u32,
        total_items: total_items as u32,
        current_page: page,
        total_pages,
        page_size,
        velocity_data_json: serde_json::to_string(&velocity_data).unwrap_or_default(),
        velocity_data,
        last_updated,
        period: "7d".to_string(),
    }
}

/// GET /dashboard/table
///
/// HTMX partial endpoint for the approaches table.
#[allow(clippy::cognitive_complexity)]
pub async fn dashboard_table(
    State(state): State<AppState>,
    Query(filters): Query<DashboardFilters>,
) -> impl IntoResponse {
    info!(
        "Dashboard table requested - page: {:?}, hazard: {:?}",
        filters.page, filters.hazard_class
    );

    let page = filters.page.unwrap_or(1);
    let page_size = 20;

    // Validate date range
    if let (Some(start), Some(end)) = (filters.start_date, filters.end_date)
        && end < start
    {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    // Fetch paginated approaches
    let (approaches, total_items) = match DashboardRepository::get_paginated_approaches(
        &state.db_pool,
        page,
        page_size,
        filters.start_date,
        filters.end_date,
        filters.hazard_class.as_deref().filter(|h| !h.is_empty()),
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to fetch approaches for dashboard table: {}", e);
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let total_pages = if total_items > 0 {
        ((total_items as f64) / (page_size as f64)).ceil() as u32
    } else {
        1
    };

    let showing_start = if total_items == 0 {
        0
    } else {
        ((page - 1) * page_size + 1) as i64
    };
    let showing_end = std::cmp::min((page * page_size) as i64, total_items);

    // Calculate page range for pagination controls
    let page_range = calculate_page_range(page, total_pages)
        .into_iter()
        .map(|p| PageItem {
            number: p,
            is_current: p == page,
        })
        .collect();

    // Build filter values for template
    let query_string = build_filter_query_string(
        filters.hazard_class.as_ref(),
        filters.start_date.as_ref(),
        filters.end_date.as_ref(),
    );
    let hazard_selected = filters
        .hazard_class
        .filter(|h| !h.is_empty())
        .unwrap_or_default();
    let start_date_value = filters
        .start_date
        .map(|d| d.to_string())
        .unwrap_or_default();
    let end_date_value = filters.end_date.map(|d| d.to_string()).unwrap_or_default();
    let is_first_page = page == 1;
    let is_last_page = page == total_pages;

    Ok::<_, axum::http::StatusCode>(ApproachesTableTemplate {
        approaches,
        showing_start: showing_start as u32,
        showing_end: showing_end as u32,
        total_items: total_items as u32,
        current_page: page,
        total_pages,
        page_size,
        page_range,
        query_string,
        hazard_selected,
        start_date_value,
        end_date_value,
        is_first_page,
        is_last_page,
    })
}

/// Formats a number with K/M suffixes for display.
pub fn format_number(num: f64) -> String {
    if num >= 1_000_000.0 {
        format!("{:.2}M", num / 1_000_000.0)
    } else if num >= 1_000.0 {
        format!("{:.1}K", num / 1_000.0)
    } else {
        format!("{:.0}", num)
    }
}

/// Calculate the page range for pagination controls.
fn calculate_page_range(current: u32, total: u32) -> Vec<u32> {
    let start = current.saturating_sub(2).max(1);
    let end = (current + 2).min(total);
    (start..=end).collect()
}

/// Build query string from filter values.
fn build_filter_query_string(
    hazard_class: Option<&String>,
    start_date: Option<&NaiveDate>,
    end_date: Option<&NaiveDate>,
) -> String {
    let mut parts = Vec::new();

    if let Some(hazard) = hazard_class.filter(|h| !h.is_empty()) {
        parts.push(format!("hazard_class={}", urlencoding::encode(hazard)));
    }
    if let Some(start) = start_date {
        parts.push(format!("start_date={}", start));
    }
    if let Some(end) = end_date {
        parts.push(format!("end_date={}", end));
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("&{}", parts.join("&"))
    }
}

/// Query parameters for velocity data refresh.
#[derive(Debug, Deserialize)]
pub struct VelocityRefreshQuery {
    /// Time period for velocity data (e.g., "7d", "30d", "90d", "1y").
    pub period: Option<TimePeriod>,
}

/// GET /dashboard/velocity
///
/// HTMX partial endpoint for refreshing the velocity chart.
pub async fn refresh_velocity_chart(
    State(state): State<AppState>,
    Query(params): Query<VelocityRefreshQuery>,
) -> impl IntoResponse {
    info!(
        "Velocity chart refresh requested with period: {:?}",
        params.period
    );

    let period = params.period.unwrap_or(TimePeriod::Last7Days);
    let period_str = match period {
        TimePeriod::Last7Days => "7d",
        TimePeriod::Last30Days => "30d",
        TimePeriod::Last90Days => "90d",
        TimePeriod::LastYear => "1y",
    };

    let velocity_data =
        match DashboardRepository::get_velocity_data_by_period(&state.db_pool, period).await {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to fetch velocity data: {}", e);
                Vec::new()
            }
        };

    let now = chrono::Local::now();
    let last_updated = now.format("%I:%M:%S %p").to_string();

    VelocityChartTemplate {
        velocity_data,
        period: period_str.to_string(),
        last_updated,
    }
}

/// GET /dashboard/metrics
///
/// HTMX partial endpoint for refreshing system metrics.
pub async fn refresh_metrics(State(state): State<AppState>) -> impl IntoResponse {
    info!("Metrics refresh requested");

    let metrics = get_metrics_summary(
        &state.prometheus_config,
        &state.grafana_cloud_prometheus_config,
        &state.db_pool,
    )
    .await;

    // Determine error rate class based on value
    let error_rate = metrics.0.error_rate_percent;
    let error_rate_class = if error_rate > 5.0 {
        "text-hazard-critical"
    } else if error_rate > 1.0 {
        "text-hazard-high"
    } else {
        "text-hazard-low"
    };

    MetricsTemplate {
        requests_per_second: format!("{:.3}", metrics.0.requests_per_second),
        error_rate_percent: format!("{:.3}%", error_rate),
        error_rate_class: error_rate_class.to_string(),
        avg_response_time_ms: format!("{:.3} ms", metrics.0.avg_response_time_ms),
        db_queries_per_second: format!("{:.3}", metrics.0.db_queries_per_second),
    }
}

#[cfg(test)]
mod dashboard_tests {
    //! API handler unit tests.

    use super::*;
    use crate::api::types::{ApiResponse, HealthResponse};

    #[tokio::test]
    async fn test_health_response_structure() {
        // Test that health response has correct structure
        let response = HealthResponse {
            status: "healthy".to_string(),
            version: "1.0.0".to_string(),
            timestamp: chrono::Utc::now(),
            database_connected: true,
        };

        assert_eq!(response.status, "healthy");
        assert_eq!(response.version, "1.0.0");
        assert!(response.database_connected);
    }

    #[test]
    fn test_api_response_success() {
        let data = "test data".to_string();
        let response = ApiResponse::success(data.clone());

        assert!(response.success);
        assert_eq!(response.data, Some(data));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<String> = ApiResponse::error_message("test error".to_string());

        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("test error".to_string()));
    }

    #[test]
    fn test_format_number_millions() {
        let result = super::format_number(1500000.0);
        assert_eq!(result, "1.50M");
    }

    #[test]
    fn test_format_number_thousands() {
        let result = super::format_number(1500.0);
        assert_eq!(result, "1.5K");
    }

    #[test]
    fn test_format_number_hundreds() {
        let result = super::format_number(500.0);
        assert_eq!(result, "500");
    }

    #[test]
    fn test_dashboard_filters_to_query_string_empty() {
        let filters = DashboardFilters::default();
        let query = filters.to_query_string();
        assert_eq!(query, "");
    }

    #[test]
    fn test_dashboard_filters_to_query_string_with_hazard() {
        let filters = DashboardFilters {
            hazard_class: Some("Critical".to_string()),
            ..Default::default()
        };
        let query = filters.to_query_string();
        assert!(query.contains("hazard_class=Critical"));
    }

    #[test]
    fn test_dashboard_filters_to_query_string_with_dates() {
        use chrono::NaiveDate;

        let filters = DashboardFilters {
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1),
            end_date: NaiveDate::from_ymd_opt(2024, 12, 31),
            ..Default::default()
        };
        let query = filters.to_query_string();
        assert!(query.contains("start_date=2024-01-01"));
        assert!(query.contains("end_date=2024-12-31"));
    }

    #[test]
    fn test_calculate_page_range() {
        let range = super::calculate_page_range(5, 10);
        assert_eq!(range, vec![3, 4, 5, 6, 7]);
    }

    #[test]
    fn test_calculate_page_range_first_page() {
        let range = super::calculate_page_range(1, 10);
        assert_eq!(range, vec![1, 2, 3]);
    }

    #[test]
    fn test_calculate_page_range_last_page() {
        let range = super::calculate_page_range(10, 10);
        assert_eq!(range, vec![8, 9, 10]);
    }

    #[test]
    fn test_build_filter_query_string_empty() {
        let query = super::build_filter_query_string(None, None, None);
        assert_eq!(query, "");
    }

    #[test]
    fn test_build_filter_query_string_with_hazard() {
        let hazard = Some("High".to_string());
        let query = super::build_filter_query_string(hazard.as_ref(), None, None);
        assert!(query.contains("hazard_class=High"));
    }
}
