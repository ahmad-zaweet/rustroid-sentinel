//! Health check endpoint handler.

use axum::{Json, extract::State};

use crate::api::types::{ApiResponse, HealthResponse};
use crate::database::dashboard::DashboardRepository;
use crate::server::AppState;

/// GET /api/health
///
/// Health check endpoint that returns service status and database connectivity.
pub async fn health(State(state): State<AppState>) -> Json<ApiResponse<HealthResponse>> {
    tracing::info!("Health check requested");
    let start = std::time::Instant::now();

    let database_connected = DashboardRepository::check_connection(&state.db_pool).await;

    let duration = start.elapsed().as_secs_f64();
    crate::metrics::record_database_query("health_check", duration, database_connected);

    let response = HealthResponse {
        status: "healthy".to_string(),
        version: state.version.clone(),
        timestamp: chrono::Utc::now(),
        database_connected,
    };

    Json(ApiResponse::success(response))
}
