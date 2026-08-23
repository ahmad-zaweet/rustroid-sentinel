//! Repository integration tests.

use crate::common::database::TestDatabase;
use anyhow::Result;
use futures_util::StreamExt;
use rustroid_sentinel::database::dashboard::DashboardRepository;
use rustroid_sentinel::database::repository::AsteroidRepository;
use rustroid_sentinel::nasa::jpl_sbdb::responses::SbdbOrbitSummary;

/// Inserts a minimal asteroid row and returns its id.
async fn seed_asteroid(
    db: &TestDatabase,
    neo_reference_id: &str,
    is_sentry_object: bool,
    sentry_checked_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<uuid::Uuid> {
    let id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO asteroids (id, neo_reference_id, name, absolute_magnitude,
         estimated_diameter_min_km, estimated_diameter_max_km, is_potentially_hazardous,
         is_sentry_object, nasa_jpl_url, sentry_checked_at, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW(), NOW())",
    )
    .bind(id)
    .bind(neo_reference_id)
    .bind(format!("Test {}", neo_reference_id))
    .bind(22.5)
    .bind(0.1)
    .bind(0.5)
    .bind(false)
    .bind(is_sentry_object)
    .bind("https://example.com")
    .bind(sentry_checked_at)
    .execute(db.pool())
    .await?;
    Ok(id)
}

#[tokio::test]
async fn test_get_stats_empty_database() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let stats = DashboardRepository::get_stats(db.pool()).await?;

    assert_eq!(stats.total_asteroids, 0);
    assert_eq!(stats.total_approaches, 0);
    assert_eq!(stats.hazardous_count, 0);

    Ok(())
}

#[tokio::test]
async fn test_get_stats_with_asteroids() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

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
    db.run_migrations().await?;

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
        "INSERT INTO approaches (id, asteroid_id, close_approach_date, epoch_date_close_approach,
         velocity_km_per_s, velocity_km_per_h, miss_distance_km, miss_distance_astronomical,
         miss_distance_lunar, orbiting_body, hazard_classification, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW())",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(asteroid_id)
    .bind(chrono::NaiveDate::from_ymd_opt(2024, 6, 15).unwrap())
    .bind(1_718_409_600_000i64)
    .bind(20.0)
    .bind(72000.0)
    .bind(500000.0)
    .bind(3.34)
    .bind(1300.0)
    .bind("Earth")
    .bind("Low")
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

#[tokio::test]
async fn test_asteroids_needing_sentry_check_excludes_non_sentry_objects() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    // Not a Sentry object: must never be a candidate, regardless of PHA status.
    seed_asteroid(&db, "20240101", false, None).await?;
    // Sentry object, never checked: candidate.
    let sentry_id = seed_asteroid(&db, "20240102", true, None).await?;

    let candidates =
        AsteroidRepository::asteroids_needing_sentry_check(db.pool(), chrono::Utc::now()).await?;

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0], (sentry_id, "20240102".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_asteroids_needing_sentry_check_respects_staleness() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let now = chrono::Utc::now();
    // Checked recently: not stale relative to a `stale_before` in the past.
    seed_asteroid(&db, "20240201", true, Some(now)).await?;
    // Checked long ago: stale, should be picked up.
    let stale_id = seed_asteroid(
        &db,
        "20240202",
        true,
        Some(now - chrono::Duration::days(60)),
    )
    .await?;

    let stale_before = now - chrono::Duration::days(30);
    let candidates =
        AsteroidRepository::asteroids_needing_sentry_check(db.pool(), stale_before).await?;

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0, stale_id);

    Ok(())
}

#[tokio::test]
async fn test_update_sentry_scales_writes_values_and_stamps_checked_at() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let matched_id = seed_asteroid(&db, "20240301", true, None).await?;
    let clear_id = seed_asteroid(&db, "20240302", true, None).await?;

    let updated = AsteroidRepository::update_sentry_scales(
        db.pool(),
        &[
            (matched_id, Some(2i16), Some(-3.5f64)),
            (clear_id, None, None),
        ],
    )
    .await?;

    assert_eq!(updated, 2);

    let row: (
        Option<i16>,
        Option<f64>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT torino_scale, palermo_scale, sentry_checked_at FROM asteroids WHERE id = $1",
    )
    .bind(matched_id)
    .fetch_one(db.pool())
    .await?;
    assert_eq!(row.0, Some(2));
    assert_eq!(row.1, Some(-3.5));
    assert!(row.2.is_some());

    let clear_row: (
        Option<i16>,
        Option<f64>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT torino_scale, palermo_scale, sentry_checked_at FROM asteroids WHERE id = $1",
    )
    .bind(clear_id)
    .fetch_one(db.pool())
    .await?;
    assert_eq!(clear_row.0, None);
    assert_eq!(clear_row.1, None);
    assert!(clear_row.2.is_some());

    Ok(())
}

#[tokio::test]
async fn test_update_sentry_scales_empty_input_is_noop() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let updated = AsteroidRepository::update_sentry_scales(db.pool(), &[]).await?;

    assert_eq!(updated, 0);

    Ok(())
}

#[tokio::test]
async fn test_asteroids_needing_orbit_check_no_row_is_a_candidate() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    // No asteroid_orbits row at all: always a candidate.
    let missing_id = seed_asteroid(&db, "20240401", false, None).await?;

    let candidates: Vec<_> =
        AsteroidRepository::stream_asteroids_needing_orbit_check(db.pool(), chrono::Utc::now())
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()?;

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0], (missing_id, "20240401".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_asteroids_needing_orbit_check_respects_staleness() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let now = chrono::Utc::now();

    let fresh_id = seed_asteroid(&db, "20240402", false, None).await?;
    sqlx::query("INSERT INTO asteroid_orbits (asteroid_id, orbit_checked_at) VALUES ($1, $2)")
        .bind(fresh_id)
        .bind(now)
        .execute(db.pool())
        .await?;

    let stale_id = seed_asteroid(&db, "20240403", false, None).await?;
    sqlx::query("INSERT INTO asteroid_orbits (asteroid_id, orbit_checked_at) VALUES ($1, $2)")
        .bind(stale_id)
        .bind(now - chrono::Duration::days(120))
        .execute(db.pool())
        .await?;

    let stale_before = now - chrono::Duration::days(90);
    let candidates: Vec<_> =
        AsteroidRepository::stream_asteroids_needing_orbit_check(db.pool(), stale_before)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()?;

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].0, stale_id);

    Ok(())
}

#[tokio::test]
async fn test_upsert_asteroid_orbits_inserts_and_stamps_checked_at() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let matched_id = seed_asteroid(&db, "20240501", false, None).await?;
    let clear_id = seed_asteroid(&db, "20240502", false, None).await?;

    let matched_summary = SbdbOrbitSummary {
        eccentricity: Some(0.19),
        semi_major_axis_au: Some(0.92),
        orbit_class: Some("Aten".to_string()),
        spectral_class: Some("Sq".to_string()),
        albedo: Some(0.24),
        ..Default::default()
    };

    let updated = AsteroidRepository::upsert_asteroid_orbits(
        db.pool(),
        &[
            (matched_id, matched_summary.clone()),
            (clear_id, SbdbOrbitSummary::default()),
        ],
    )
    .await?;

    assert_eq!(updated, 2);

    type OrbitRow = (
        Option<f64>,
        Option<f64>,
        Option<String>,
        Option<String>,
        Option<f64>,
    );
    let row: OrbitRow = sqlx::query_as(
        "SELECT eccentricity, semi_major_axis_au, orbit_class, spectral_class, albedo
             FROM asteroid_orbits WHERE asteroid_id = $1",
    )
    .bind(matched_id)
    .fetch_one(db.pool())
    .await?;
    assert_eq!(row.0, matched_summary.eccentricity);
    assert_eq!(row.1, matched_summary.semi_major_axis_au);
    assert_eq!(row.2, matched_summary.orbit_class);
    assert_eq!(row.3, matched_summary.spectral_class);
    assert_eq!(row.4, matched_summary.albedo);

    let clear_row: (Option<f64>,) =
        sqlx::query_as("SELECT eccentricity FROM asteroid_orbits WHERE asteroid_id = $1")
            .bind(clear_id)
            .fetch_one(db.pool())
            .await?;
    assert_eq!(clear_row.0, None);

    Ok(())
}

#[tokio::test]
async fn test_upsert_asteroid_orbits_overwrites_on_conflict() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let id = seed_asteroid(&db, "20240503", false, None).await?;

    AsteroidRepository::upsert_asteroid_orbits(
        db.pool(),
        &[(
            id,
            SbdbOrbitSummary {
                eccentricity: Some(0.1),
                ..Default::default()
            },
        )],
    )
    .await?;

    AsteroidRepository::upsert_asteroid_orbits(
        db.pool(),
        &[(
            id,
            SbdbOrbitSummary {
                eccentricity: Some(0.9),
                ..Default::default()
            },
        )],
    )
    .await?;

    let row: (Option<f64>,) =
        sqlx::query_as("SELECT eccentricity FROM asteroid_orbits WHERE asteroid_id = $1")
            .bind(id)
            .fetch_one(db.pool())
            .await?;
    assert_eq!(row.0, Some(0.9));

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM asteroid_orbits WHERE asteroid_id = $1")
            .bind(id)
            .fetch_one(db.pool())
            .await?;
    assert_eq!(count.0, 1);

    Ok(())
}

#[tokio::test]
async fn test_upsert_asteroid_orbits_empty_input_is_noop() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let updated = AsteroidRepository::upsert_asteroid_orbits(db.pool(), &[]).await?;

    assert_eq!(updated, 0);

    Ok(())
}
