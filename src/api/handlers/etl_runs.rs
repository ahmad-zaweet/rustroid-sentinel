//! ETL runs endpoint handlers.

use axum::{Json, extract::Query, extract::State};
use serde::Deserialize;
use tracing::{error, info};

use crate::api::templates::EtlRunsTemplate;
use crate::api::types::{ApiResponse, EtlRunsResponse};
use crate::database::dashboard::DashboardRepository;
use crate::server::AppState;

/// Query parameters for paginated ETL runs.
#[derive(Debug, Deserialize)]
pub struct EtlRunsQuery {
    /// The page number to retrieve (1-indexed).
    #[serde(default = "default_page")]
    pub page: u32,
    /// The number of items to return per page.
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    10
}

/// GET /api/etl-runs
///
/// Returns recent ETL run history (API endpoint).
pub async fn etl_runs(State(state): State<AppState>) -> Json<ApiResponse<EtlRunsResponse>> {
    info!("ETL runs requested");

    let runs = match DashboardRepository::get_recent_etl_runs(&state.db_pool).await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to fetch ETL runs: {}", e);
            return Json(ApiResponse::error_message(format!("Database error: {}", e)));
        }
    };

    Json(ApiResponse::success(EtlRunsResponse { runs }))
}

/// GET /dashboard/etl-runs
///
/// HTMX partial endpoint for ETL runs with pagination.
pub async fn dashboard_etl_runs(
    State(state): State<AppState>,
    Query(params): Query<EtlRunsQuery>,
) -> impl axum::response::IntoResponse {
    info!("ETL runs partial requested - page: {}", params.page);

    let page_size = params.page_size.min(20); // Cap at 20

    // Fetch paginated ETL runs
    let (runs, total_count) =
        match DashboardRepository::get_paginated_etl_runs(&state.db_pool, params.page, page_size)
            .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to fetch ETL runs for dashboard: {}", e);
                return EtlRunsTemplate {
                    etl_runs: vec![],
                    current_page: params.page,
                    total_pages: 1,
                    page_size,
                };
            }
        };

    let total_pages = if total_count > 0 {
        ((total_count as f64) / (page_size as f64)).ceil() as u32
    } else {
        1
    };

    EtlRunsTemplate {
        etl_runs: runs,
        current_page: params.page,
        total_pages,
        page_size,
    }
}
