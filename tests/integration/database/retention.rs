//! Retention/pruning repository integration tests.

use crate::common::database::TestDatabase;
use anyhow::Result;
use chrono::{Duration, Utc};
use rustroid_sentinel::database::retention::RetentionRepository;
use rustroid_sentinel::settings::RetentionConfig;

fn test_retention_config() -> RetentionConfig {
    RetentionConfig {
        approach_retention_years: 1,
        etl_event_retention_days: 30,
        etl_events_keep_min: 2,
    }
}

async fn seed_asteroid(db: &TestDatabase) -> Result<uuid::Uuid> {
    let asteroid_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO asteroids (id, neo_reference_id, name, absolute_magnitude,
         estimated_diameter_min_km, estimated_diameter_max_km, is_potentially_hazardous,
         is_sentry_object, nasa_jpl_url, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())",
    )
    .bind(asteroid_id)
    .bind(asteroid_id.to_string())
    .bind("Test Asteroid")
    .bind(22.5)
    .bind(0.1)
    .bind(0.5)
    .bind(false)
    .bind(false)
    .bind("https://example.com")
    .execute(db.pool())
    .await?;
    Ok(asteroid_id)
}

async fn seed_approach(
    db: &TestDatabase,
    asteroid_id: uuid::Uuid,
    close_approach_date: chrono::NaiveDate,
    epoch: i64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO approaches (id, asteroid_id, close_approach_date, epoch_date_close_approach,
         velocity_km_per_s, velocity_km_per_h, miss_distance_km, miss_distance_astronomical,
         miss_distance_lunar)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(asteroid_id)
    .bind(close_approach_date)
    .bind(epoch)
    .bind(10.0)
    .bind(36000.0)
    .bind(1_000_000.0)
    .bind(0.01)
    .bind(2.5)
    .execute(db.pool())
    .await?;
    Ok(())
}

async fn seed_etl_event(
    db: &TestDatabase,
    source_file: &str,
    started_at: chrono::DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO etl_events (id, source_file, started_at, completed_at, status)
         VALUES ($1, $2, $3, $3, 'success')",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(source_file)
    .bind(started_at)
    .execute(db.pool())
    .await?;
    Ok(())
}

#[tokio::test]
async fn test_count_prunable_empty_database() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let result = RetentionRepository::count_prunable(db.pool(), &test_retention_config()).await?;

    assert_eq!(result.approaches, 0);
    assert_eq!(result.etl_events, 0);

    Ok(())
}

#[tokio::test]
async fn test_count_and_prune_approaches_by_age() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let asteroid_id = seed_asteroid(&db).await?;
    let old_date = (Utc::now() - Duration::days(400)).date_naive();
    let recent_date = Utc::now().date_naive();

    seed_approach(&db, asteroid_id, old_date, 1).await?;
    seed_approach(&db, asteroid_id, recent_date, 2).await?;

    let config = test_retention_config();

    let counted = RetentionRepository::count_prunable(db.pool(), &config).await?;
    assert_eq!(counted.approaches, 1, "only the >1yr-old approach counts");

    let pruned = RetentionRepository::prune(db.pool(), &config).await?;
    assert_eq!(pruned.approaches, 1);

    let (remaining,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM approaches")
        .fetch_one(db.pool())
        .await?;
    assert_eq!(remaining, 1, "recent approach must survive pruning");

    // Idempotent: pruning again finds nothing left to delete.
    let second_pass = RetentionRepository::prune(db.pool(), &config).await?;
    assert_eq!(second_pass.approaches, 0);

    Ok(())
}

#[tokio::test]
async fn test_prune_etl_events_respects_keep_min() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let config = test_retention_config(); // etl_events_keep_min = 2, retention = 30 days

    // 4 events, all older than the 30-day retention window, oldest first.
    for i in 0..4 {
        let started_at = Utc::now() - Duration::days(100 - i64::from(i));
        seed_etl_event(&db, &format!("batch-{i}.json"), started_at).await?;
    }

    let counted = RetentionRepository::count_prunable(db.pool(), &config).await?;
    // 4 total, minus the 2 most recent that keep_min always protects.
    assert_eq!(counted.etl_events, 2);

    let pruned = RetentionRepository::prune(db.pool(), &config).await?;
    assert_eq!(pruned.etl_events, 2);

    let (remaining,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM etl_events")
        .fetch_one(db.pool())
        .await?;
    assert_eq!(
        remaining, 2,
        "keep_min rows must survive despite being stale"
    );

    Ok(())
}

#[tokio::test]
async fn test_prune_etl_events_keeps_recent_regardless_of_keep_min() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let config = test_retention_config(); // retention = 30 days

    // All events are recent (well within the retention window), so nothing
    // should be pruned even though there are more rows than keep_min.
    for i in 0..5 {
        let started_at = Utc::now() - Duration::hours(i64::from(i));
        seed_etl_event(&db, &format!("recent-{i}.json"), started_at).await?;
    }

    let pruned = RetentionRepository::prune(db.pool(), &config).await?;
    assert_eq!(pruned.etl_events, 0);

    Ok(())
}
