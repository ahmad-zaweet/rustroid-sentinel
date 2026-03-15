//! Stats endpoint handler.

use axum::{Json, extract::State};
use tracing::{error, info};

use crate::api::types::{ApiResponse, StatsResponse};
use crate::database::dashboard::DashboardRepository;
use crate::server::AppState;

/// GET /api/stats
///
/// Returns a high-level summary of asteroid statistics and recent approaches.
#[allow(clippy::cognitive_complexity)]
pub async fn stats(State(state): State<AppState>) -> Json<ApiResponse<StatsResponse>> {
    info!("Stats requested");
    let start = std::time::Instant::now();

    // Get dashboard statistics
    let stats = match DashboardRepository::get_stats(&state.db_pool).await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to fetch dashboard stats: {}", e);
            return Json(ApiResponse::error_message(format!("Database error: {}", e)));
        }
    };

    // Get recent approaches
    let recent_approaches = match DashboardRepository::get_recent_approaches(&state.db_pool).await {
        Ok(a) => a,
        Err(e) => {
            error!("Failed to fetch recent approaches: {}", e);
            return Json(ApiResponse::error_message(format!("Database error: {}", e)));
        }
    };

    // Get velocity data
    let velocity_data = match DashboardRepository::get_velocity_data(&state.db_pool).await {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to fetch velocity data: {}", e);
            return Json(ApiResponse::error_message(format!("Database error: {}", e)));
        }
    };

    let duration = start.elapsed().as_secs_f64();
    crate::metrics::record_database_query("get_stats", duration, true);

    let response = StatsResponse {
        total_asteroids: stats.total_asteroids,
        total_approaches: stats.total_approaches,
        hazardous_count: stats.hazardous_count,
        recent_approaches,
        velocity_data,
    };

    Json(ApiResponse::success(response))
}
