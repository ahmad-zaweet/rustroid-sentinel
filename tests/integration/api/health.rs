//! Health endpoint integration tests.

use crate::common::database::TestDatabase;
use anyhow::Result;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use rustroid_sentinel::{api::routes::api_router, server::AppState, settings::ServerConfig};
use tower::ServiceExt; // for `oneshot` and `ready`

#[tokio::test]
async fn test_health_endpoint_returns_200() -> Result<()> {
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
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let body: serde_json::Value = serde_json::from_slice(&body_bytes)?;

    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], "2.1.0");
    assert_eq!(body["database"], "up");

    Ok(())
}
