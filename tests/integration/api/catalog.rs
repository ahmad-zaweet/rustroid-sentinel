//! Asteroid catalog endpoint integration tests: exercises pagination and
//! filtering over real HTTP requests, not just the repository layer.

use crate::common::database::TestDatabase;
use anyhow::Result;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::NaiveDate;
use rustroid_sentinel::{api::routes::api_router, server::AppState, settings::ServerConfig};
use tower::ServiceExt;

fn test_app_state(db: &TestDatabase) -> AppState {
    AppState::new(
        db.pool().clone(),
        ServerConfig {
            request_timeout_seconds: 30,
            rate_limit_requests: 100,
            rate_limit_period_seconds: 60,
            max_hazard_subscribers: 100,
            internal_event_rate_limit_requests: 30,
            cache: Default::default(),
        },
        env!("CARGO_PKG_VERSION").to_string(),
        None,
        None,
        rustroid_sentinel::events::channel(),
        "test-token".into(),
        tokio::sync::watch::channel(false).1,
    )
}

async fn seed_asteroid(
    db: &TestDatabase,
    neo_reference_id: &str,
    name: &str,
) -> Result<uuid::Uuid> {
    let id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO asteroids (id, neo_reference_id, name, absolute_magnitude,
         estimated_diameter_min_km, estimated_diameter_max_km, is_potentially_hazardous,
         is_sentry_object, nasa_jpl_url, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())",
    )
    .bind(id)
    .bind(neo_reference_id)
    .bind(name)
    .bind(22.5)
    .bind(0.1)
    .bind(0.5)
    .bind(false)
    .bind(false)
    .bind("https://example.com")
    .execute(db.pool())
    .await?;
    Ok(id)
}

async fn seed_approach(db: &TestDatabase, asteroid_id: uuid::Uuid, date: NaiveDate) -> Result<()> {
    sqlx::query(
        "INSERT INTO approaches (id, asteroid_id, close_approach_date, epoch_date_close_approach,
         velocity_km_per_s, velocity_km_per_h, miss_distance_km, miss_distance_astronomical,
         miss_distance_lunar, orbiting_body, hazard_classification, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW())",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(asteroid_id)
    .bind(date)
    .bind(
        date.and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis(),
    )
    .bind(20.0)
    .bind(72000.0)
    .bind(500000.0)
    .bind(3.34)
    .bind(1300.0)
    .bind("Earth")
    .bind("Low")
    .execute(db.pool())
    .await?;
    Ok(())
}

/// A filter form with only `name` filled in still submits every other
/// input's value, which for a blank `<input type="date">` is `""` — this
/// must not 400 (the bug `hx-params="not empty"` was mistakenly relied on
/// to prevent, but that's not a real htmx feature).
#[tokio::test]
async fn test_asteroids_endpoint_tolerates_empty_filter_fields() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;
    seed_asteroid(&db, "40240101", "Apophis").await?;

    let app = api_router().with_state(test_app_state(&db));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/asteroids?name=apophis&start_date=&end_date=&orbit_class=&spectral_class=&min_diameter_km=&max_diameter_km=&min_torino_scale=&min_palermo_scale=")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let body: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    assert_eq!(body["data"]["data"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"]["data"][0]["name"], "Apophis");

    Ok(())
}

/// Full pagination walk over HTTP: page size 1 via `min_torino_scale`
/// untouched, 3 seeded asteroids, follow `next_cursor` until it's `None`
/// and confirm every asteroid was seen exactly once.
#[tokio::test]
async fn test_asteroids_endpoint_paginates_via_next_cursor() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let mut seeded_ids = Vec::new();
    for i in 0..3 {
        let id = seed_asteroid(&db, &format!("4025010{i}"), &format!("Test{i}")).await?;
        seed_approach(
            &db,
            id,
            NaiveDate::from_ymd_opt(2026, 2, 1).unwrap() + chrono::Duration::days(i),
        )
        .await?;
        seeded_ids.push(id.to_string());
    }

    let app = api_router().with_state(test_app_state(&db));

    let mut seen_ids = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let uri = match &cursor {
            Some(c) => format!("/asteroids?cursor={c}"),
            None => "/asteroids".to_string(),
        };
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
        let body: serde_json::Value = serde_json::from_slice(&body_bytes)?;
        for row in body["data"]["data"].as_array().unwrap() {
            seen_ids.push(row["id"].as_str().unwrap().to_string());
        }

        cursor = body["data"]["next_cursor"].as_str().map(str::to_string);
        if cursor.is_none() {
            break;
        }
    }

    seen_ids.sort();
    let mut expected = seeded_ids.clone();
    expected.sort();
    assert_eq!(
        seen_ids, expected,
        "every seeded asteroid seen exactly once across pages"
    );

    Ok(())
}

#[tokio::test]
async fn test_asteroids_endpoint_rejects_malformed_cursor() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let app = api_router().with_state(test_app_state(&db));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/asteroids?cursor=not-valid-base64!!")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

async fn seed_embedding(
    db: &TestDatabase,
    asteroid_id: uuid::Uuid,
    values: [f32; 16],
) -> Result<()> {
    sqlx::query(
        "INSERT INTO asteroid_embeddings (asteroid_id, embedding, computed_at)
         VALUES ($1, $2, NOW())",
    )
    .bind(asteroid_id)
    .bind(pgvector::Vector::from(values.to_vec()))
    .execute(db.pool())
    .await?;
    Ok(())
}

#[tokio::test]
async fn test_similar_endpoint_returns_nearest_neighbors() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let origin = seed_asteroid(&db, "40250701", "Origin").await?;
    seed_embedding(&db, origin, [0.0; 16]).await?;

    let near = seed_asteroid(&db, "40250702", "Near").await?;
    seed_embedding(&db, near, [0.1; 16]).await?;

    let far = seed_asteroid(&db, "40250703", "Far").await?;
    seed_embedding(&db, far, [0.9; 16]).await?;

    let app = api_router().with_state(test_app_state(&db));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/asteroids/40250701/similar")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let body: serde_json::Value = serde_json::from_slice(&body_bytes)?;
    let rows = body["data"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["name"], "Near");
    assert_eq!(rows[1]["name"], "Far");

    Ok(())
}

#[tokio::test]
async fn test_similar_endpoint_404s_for_unvectorized_asteroid() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;
    seed_asteroid(&db, "40250801", "Unvectorized").await?;

    let app = api_router().with_state(test_app_state(&db));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/asteroids/40250801/similar")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    Ok(())
}
