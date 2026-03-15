//! Velocity data endpoint handler.

use axum::{Json, extract::Query, extract::State};
use chrono::NaiveDate;
use serde::Deserialize;
use tracing::{error, info};

use crate::api::types::{ApiResponse, TimePeriod, VelocityDataPoint};
use crate::database::dashboard::DashboardRepository;
use crate::server::AppState;

/// Query parameters for velocity data with timeline filtering.
#[derive(Debug, Deserialize)]
pub struct VelocityQuery {
    /// Predefined time period variant (e.g., "7d", "30d", "90d", "1y").
    pub period: Option<TimePeriod>,
    /// Optional custom start date. Format: YYYY-MM-DD
    pub start_date: Option<NaiveDate>,
    /// Optional custom end date. Format: YYYY-MM-DD
    pub end_date: Option<NaiveDate>,
}

/// GET /api/velocity
///
/// Returns velocity data with timeline filtering.
#[allow(clippy::cognitive_complexity)]
pub async fn velocity(
    State(state): State<AppState>,
    Query(params): Query<VelocityQuery>,
) -> Json<ApiResponse<Vec<VelocityDataPoint>>> {
    info!("Velocity data requested with period: {:?}", params.period);

    let velocity_data = if let Some(period) = params.period {
        match DashboardRepository::get_velocity_data_by_period(&state.db_pool, period).await {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to fetch velocity data: {}", e);
                return Json(ApiResponse::error_message(format!("Database error: {}", e)));
            }
        }
    } else if params.start_date.is_some() || params.end_date.is_some() {
        match DashboardRepository::get_velocity_data_with_filter(
            &state.db_pool,
            params.start_date,
            params.end_date,
        )
        .await
        {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to fetch velocity data: {}", e);
                return Json(ApiResponse::error_message(format!("Database error: {}", e)));
            }
        }
    } else {
        // Default to 90 days
        match DashboardRepository::get_velocity_data_by_period(
            &state.db_pool,
            TimePeriod::Last7Days,
        )
        .await
        {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to fetch velocity data: {}", e);
                return Json(ApiResponse::error_message(format!("Database error: {}", e)));
            }
        }
    };

    Json(ApiResponse::success(velocity_data))
}
