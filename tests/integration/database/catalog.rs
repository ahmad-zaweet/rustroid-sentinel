//! Catalog repository integration tests.

use crate::common::database::TestDatabase;
use anyhow::Result;
use chrono::NaiveDate;
use rustroid_sentinel::api::cursor::{CatalogCursor, CursorValue};
use rustroid_sentinel::api::types::{CatalogSortKey, SortDir};
use rustroid_sentinel::database::catalog::{CatalogListParams, CatalogRepository};

/// Inserts a minimal asteroid row and returns its id.
async fn seed_asteroid(
    db: &TestDatabase,
    neo_reference_id: &str,
    is_potentially_hazardous: bool,
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
    .bind(format!("Test {}", neo_reference_id))
    .bind(22.5)
    .bind(0.1)
    .bind(0.5)
    .bind(is_potentially_hazardous)
    .bind(false)
    .bind("https://example.com")
    .execute(db.pool())
    .await?;
    Ok(id)
}

/// Inserts an approach row for `asteroid_id` on `date`.
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

/// Inserts an `asteroid_orbits` row for `asteroid_id`.
async fn seed_orbit(
    db: &TestDatabase,
    asteroid_id: uuid::Uuid,
    orbit_class: &str,
    spectral_class: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO asteroid_orbits (asteroid_id, orbit_class, spectral_class, albedo, orbit_checked_at)
         VALUES ($1, $2, $3, $4, NOW())",
    )
    .bind(asteroid_id)
    .bind(orbit_class)
    .bind(spectral_class)
    .bind(0.2)
    .execute(db.pool())
    .await?;
    Ok(())
}

fn default_params(limit: u32) -> CatalogListParams<'static> {
    CatalogListParams {
        sort: CatalogSortKey::ApproachActivity,
        limit,
        ..Default::default()
    }
}

#[tokio::test]
async fn test_list_orders_by_latest_approach_date_desc_nulls_last() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let no_approach = seed_asteroid(&db, "30240101", false).await?;
    let older = seed_asteroid(&db, "30240102", false).await?;
    seed_approach(&db, older, NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()).await?;
    let newer = seed_asteroid(&db, "30240103", false).await?;
    seed_approach(&db, newer, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()).await?;

    let (rows, has_more) = CatalogRepository::list(db.pool(), default_params(10)).await?;

    assert!(!has_more);
    let ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    assert_eq!(ids, vec![newer, older, no_approach]);

    Ok(())
}

#[tokio::test]
async fn test_list_pagination_cursor_continues_without_repeats() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let mut expected_ids = Vec::new();
    for i in 0..3 {
        let id = seed_asteroid(&db, &format!("3025010{i}"), false).await?;
        seed_approach(
            &db,
            id,
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() + chrono::Duration::days(i),
        )
        .await?;
        expected_ids.push(id);
    }
    expected_ids.reverse(); // DESC order: most recent approach first

    let (page1, has_more1) = CatalogRepository::list(db.pool(), default_params(2)).await?;
    assert!(has_more1);
    assert_eq!(page1.len(), 2);
    assert_eq!(page1[0].id, expected_ids[0]);
    assert_eq!(page1[1].id, expected_ids[1]);

    let cursor = CatalogCursor {
        sort: CatalogSortKey::ApproachActivity,
        sort_dir: SortDir::Desc,
        value: CursorValue::Date(page1[1].latest_approach_date),
        id: page1[1].id,
    };
    let (page2, has_more2) = CatalogRepository::list(
        db.pool(),
        CatalogListParams {
            cursor: Some(cursor),
            ..default_params(2)
        },
    )
    .await?;

    assert!(!has_more2);
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0].id, expected_ids[2]);

    Ok(())
}

#[tokio::test]
async fn test_list_filters_by_name() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    seed_asteroid(&db, "30240201", false).await?;
    let apophis_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO asteroids (id, neo_reference_id, name, absolute_magnitude,
         estimated_diameter_min_km, estimated_diameter_max_km, is_potentially_hazardous,
         is_sentry_object, nasa_jpl_url, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())",
    )
    .bind(apophis_id)
    .bind("30240202")
    .bind("99942 Apophis")
    .bind(19.7)
    .bind(0.3)
    .bind(0.4)
    .bind(true)
    .bind(false)
    .bind("https://example.com")
    .execute(db.pool())
    .await?;

    let (rows, _) = CatalogRepository::list(
        db.pool(),
        CatalogListParams {
            name: Some("apophis"),
            ..default_params(10)
        },
    )
    .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, apophis_id);

    Ok(())
}

#[tokio::test]
async fn test_list_filters_by_hazardous_and_orbit_class() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let hazardous_aten = seed_asteroid(&db, "30240301", true).await?;
    seed_orbit(&db, hazardous_aten, "Aten", "Sq").await?;

    let hazardous_amor = seed_asteroid(&db, "30240302", true).await?;
    seed_orbit(&db, hazardous_amor, "Amor", "S").await?;

    seed_asteroid(&db, "30240303", false).await?;

    let (rows, _) = CatalogRepository::list(
        db.pool(),
        CatalogListParams {
            is_potentially_hazardous: Some(true),
            orbit_class: Some("Aten"),
            ..default_params(10)
        },
    )
    .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, hazardous_aten);

    Ok(())
}

#[tokio::test]
async fn test_get_detail_returns_summary_orbit_and_approaches() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let id = seed_asteroid(&db, "30240401", true).await?;
    seed_orbit(&db, id, "Aten", "Sq").await?;
    seed_approach(&db, id, NaiveDate::from_ymd_opt(2026, 3, 1).unwrap()).await?;
    seed_approach(&db, id, NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()).await?;

    let detail = CatalogRepository::get_detail(db.pool(), "30240401")
        .await?
        .expect("expected a detail record");

    assert_eq!(detail.summary.id, id);
    assert_eq!(detail.summary.orbit_class.as_deref(), Some("Aten"));
    assert_eq!(detail.albedo, Some(0.2));
    assert_eq!(detail.approaches.len(), 2);
    // Most recent first.
    assert_eq!(
        detail.approaches[0].close_approach_date,
        NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()
    );

    Ok(())
}

#[tokio::test]
async fn test_get_detail_returns_none_for_unknown_id() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let detail = CatalogRepository::get_detail(db.pool(), "does-not-exist").await?;

    assert!(detail.is_none());

    Ok(())
}

/// Inserts an `asteroid_embeddings` row for `asteroid_id`.
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
async fn test_similar_orders_by_distance_and_excludes_self() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    let origin = seed_asteroid(&db, "30250501", false).await?;
    seed_embedding(&db, origin, [0.0; 16]).await?;

    let near = seed_asteroid(&db, "30250502", false).await?;
    seed_embedding(&db, near, [0.1; 16]).await?;

    let far = seed_asteroid(&db, "30250503", false).await?;
    seed_embedding(&db, far, [0.9; 16]).await?;

    let rows = CatalogRepository::similar(db.pool(), "30250501", 10)
        .await?
        .expect("origin is vectorized, should return Some");

    let ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    assert_eq!(ids, vec![near, far]);
    assert!(!ids.contains(&origin));

    Ok(())
}

#[tokio::test]
async fn test_similar_returns_none_when_not_vectorized() -> Result<()> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;

    seed_asteroid(&db, "30250601", false).await?;

    let rows = CatalogRepository::similar(db.pool(), "30250601", 10).await?;

    assert!(rows.is_none());

    Ok(())
}
