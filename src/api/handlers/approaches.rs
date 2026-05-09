//! Approaches endpoint handler.

use axum::{Json, extract::Query, extract::State};
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::{error, info};

use crate::api::types::{ApiResponse, ApproachRecord, PaginatedResponse, PaginationInfo};
use crate::database::dashboard::DashboardRepository;
use crate::server::AppState;

/// Query parameters for paginated approaches.
#[derive(Debug, Deserialize)]
pub struct ApproachesQuery {
    /// The page number to retrieve (1-indexed).
    #[serde(default = "default_page")]
    pub page: u32,
    /// The number of items to return per page.
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    /// Optional start date (inclusive). Format: YYYY-MM-DD
    pub start_date: Option<NaiveDate>,
    /// Optional end date (inclusive). Format: YYYY-MM-DD
    pub end_date: Option<NaiveDate>,
    /// Optional hazard classification filter.
    pub hazard_class: Option<String>,
}

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    20
}

/// GET /api/approaches
///
/// Returns paginated approach records with filtering.
pub async fn approaches(
    State(state): State<AppState>,
    Query(params): Query<ApproachesQuery>,
) -> Json<ApiResponse<PaginatedResponse<ApproachRecord>>> {
    info!(
        "Approaches requested - page: {}, size: {}",
        params.page, params.page_size
    );

    let (approaches, total_items) = match DashboardRepository::get_paginated_approaches(
        &state.db_pool,
        crate::database::dashboard::ApproachQueryParams {
            page: params.page,
            page_size: params.page_size,
            start_date: params.start_date,
            end_date: params.end_date,
            hazard_class: params.hazard_class.as_deref(),
            sort_by: None,
            sort_dir: None,
        },
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to fetch approaches: {}", e);
            return Json(ApiResponse::error_message(format!("Database error: {}", e)));
        }
    };

    let total_pages = ((total_items as f64) / (params.page_size as f64)).ceil() as u32;

    let response = PaginatedResponse {
        data: approaches,
        pagination: PaginationInfo {
            page: params.page,
            page_size: params.page_size,
            total_items,
            total_pages,
        },
    };

    Json(ApiResponse::success(response))
}
