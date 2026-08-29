//! Stats endpoint integration tests.

use crate::common::database::TestDatabase;
use anyhow::Result;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use rustroid_sentinel::{api::routes::api_router, server::AppState, settings::ServerConfig};
use tower::ServiceExt; // for `oneshot` and `ready`

#[tokio::test]
async fn test_stats_endpoint_returns_asteroid_counts() -> Result<()> {
    // Setup test database
    let db = TestDatabase::new().await?;

    // Seed test data
    sqlx::query("INSERT INTO asteroids (id, neo_reference_id, name, absolute_magnitude, estimated_diameter_min_km, estimated_diameter_max_km, is_potentially_hazardous, is_sentry_object, nasa_jpl_url, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())")
        .bind(uuid::Uuid::new_v4())
        .bind("20240101")
        .bind("Test Asteroid")
        .bind(22.5)
        .bind(0.1)
        .bind(0.5)
        .bind(false)
        .bind(false)
        .bind("https://example.com")
        .execute(db.pool())
        .await?;

    let app_state = AppState::new(
        db.pool().clone(),
        ServerConfig {
            request_timeout_seconds: 30,
            rate_limit_requests: 100,
            rate_limit_period_seconds: 60,
            max_hazard_subscribers: 100,
            internal_event_rate_limit_requests: 30,
        },
        "2.1.0".to_string(),
        None,
        None,
        rustroid_sentinel::events::channel(),
        "test-token".into(),
        tokio::sync::watch::channel(false).1,
    );

    let app = api_router().with_state(app_state);

    // Act
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let body: serde_json::Value = serde_json::from_slice(&body_bytes)?;

    assert_eq!(body["total_asteroids"], 1);

    Ok(())
}

#[tokio::test]
async fn test_stats_endpoint_empty_database() -> Result<()> {
    // Setup test database
    let db = TestDatabase::new().await?;

    let app_state = AppState::new(
        db.pool().clone(),
        ServerConfig {
            request_timeout_seconds: 30,
            rate_limit_requests: 100,
            rate_limit_period_seconds: 60,
            max_hazard_subscribers: 100,
            internal_event_rate_limit_requests: 30,
        },
        "2.1.0".to_string(),
        None,
        None,
        rustroid_sentinel::events::channel(),
        "test-token".into(),
        tokio::sync::watch::channel(false).1,
    );

    let app = api_router().with_state(app_state);

    // Act
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let body: serde_json::Value = serde_json::from_slice(&body_bytes)?;

    assert_eq!(body["total_asteroids"], 0);

    Ok(())
}
