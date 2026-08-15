//! Repository integration tests.

use crate::common::database::TestDatabase;
use anyhow::Result;
use rustroid_sentinel::database::dashboard::DashboardRepository;

#[tokio::test]
async fn test_get_stats_empty_database() -> Result<()> {
    let db = TestDatabase::new().await?;

    let stats = DashboardRepository::get_stats(db.pool()).await?;

    assert_eq!(stats.total_asteroids, 0);
    assert_eq!(stats.total_approaches, 0);
    assert_eq!(stats.hazardous_count, 0);

    Ok(())
}

#[tokio::test]
async fn test_get_stats_with_asteroids() -> Result<()> {
    let db = TestDatabase::new().await?;

    // Seed test data
    sqlx::query(
        "INSERT INTO asteroids (id, neo_reference_id, name, absolute_magnitude,
         estimated_diameter_min_km, estimated_diameter_max_km, is_potentially_hazardous,
         is_sentry_object, nasa_jpl_url, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())",
    )
    .bind(uuid::Uuid::new_v4())
    .bind("20240101")
    .bind("Test Asteroid 1")
    .bind(22.5)
    .bind(0.1)
    .bind(0.5)
    .bind(true) // hazardous
    .bind(false)
    .bind("https://example.com")
    .execute(db.pool())
    .await?;

    sqlx::query(
        "INSERT INTO asteroids (id, neo_reference_id, name, absolute_magnitude,
         estimated_diameter_min_km, estimated_diameter_max_km, is_potentially_hazardous,
         is_sentry_object, nasa_jpl_url, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())",
    )
    .bind(uuid::Uuid::new_v4())
    .bind("20240102")
    .bind("Test Asteroid 2")
    .bind(20.0)
    .bind(0.5)
    .bind(1.0)
    .bind(false) // not hazardous
    .bind(false)
    .bind("https://example.com")
    .execute(db.pool())
    .await?;

    let stats = DashboardRepository::get_stats(db.pool()).await?;

    assert_eq!(stats.total_asteroids, 2);
    assert_eq!(stats.hazardous_count, 1);

    Ok(())
}

#[tokio::test]
async fn test_get_stats_with_approaches() -> Result<()> {
    let db = TestDatabase::new().await?;

    // Seed asteroid
    let asteroid_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO asteroids (id, neo_reference_id, name, absolute_magnitude,
         estimated_diameter_min_km, estimated_diameter_max_km, is_potentially_hazardous,
         is_sentry_object, nasa_jpl_url, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())",
    )
    .bind(asteroid_id)
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

    // Seed approaches
    sqlx::query(
        "INSERT INTO approaches (id, asteroid_id, close_approach_date, velocity_km_per_h,
         miss_distance_km, orbiting_body, hazard_classification, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(asteroid_id)
    .bind(chrono::NaiveDate::from_ymd_opt(2024, 6, 15).unwrap())
    .bind(72000.0)
    .bind(500000.0)
    .bind("Earth")
    .bind(None::<String>)
    .execute(db.pool())
    .await?;

    let stats = DashboardRepository::get_stats(db.pool()).await?;

    assert_eq!(stats.total_approaches, 1);

    Ok(())
}

#[tokio::test]
async fn test_check_connection_success() {
    let db = TestDatabase::new().await.unwrap();

    let is_connected = DashboardRepository::check_connection(db.pool()).await;

    assert!(is_connected);
}
