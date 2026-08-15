//! Write-heavy database operations managing the core domain models.
//!
//! Handles UPSERTs of asteroids and approaches in performant batches using `sqlx`.

use crate::events::HazardEvent;
use crate::models::HazardClassification;
use crate::models::approach::Approach;
use crate::models::asteroid::Asteroid;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{debug, warn};
use uuid::Uuid;

/// Statistics returned from an upsert operation.
///
/// Tracks the number of records affected during a batch database operation.
#[derive(Debug, Default)]
pub struct UpsertStats {
    /// Number of new asteroids created.
    pub asteroids_inserted: u64,
    /// Number of existing asteroids updated with new metadata.
    pub asteroids_updated: u64,
    /// Number of new close approach events recorded.
    pub approaches_inserted: u64,
    /// Number of duplicate approach events that were ignored.
    pub approaches_skipped: u64,
    /// Newly-inserted approaches classified `Critical` or `High`, for
    /// publishing to the hazard event stream. Empty for duplicate rows that
    /// `ON CONFLICT ... DO NOTHING` skipped.
    pub new_hazard_events: Vec<HazardEvent>,
}

impl std::fmt::Display for UpsertStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Asteroids: {} inserted, {} updated | Approaches: {} inserted, {} skipped",
            self.asteroids_inserted,
            self.asteroids_updated,
            self.approaches_inserted,
            self.approaches_skipped
        )
    }
}

/// Repository for persisting asteroid and approach data to PostgreSQL.
///
/// All operations use `INSERT ... ON CONFLICT` to ensure idempotent behavior.
pub struct AsteroidRepository;

impl AsteroidRepository {
    /// Upserts a batch of asteroids and their approaches within a single transaction.
    ///
    /// - Asteroids are upserted via `INSERT ... ON CONFLICT (neo_reference_id) DO UPDATE`.
    /// - Approaches use `INSERT ... ON CONFLICT (asteroid_id, epoch_date_close_approach) DO NOTHING`.
    ///
    /// This ensures:
    /// - Asteroid metadata is always up-to-date (UPDATE on conflict).
    /// - Duplicate approach records are silently skipped (DO NOTHING).
    ///
    /// # Arguments
    ///
    /// * `pool` - The database connection pool.
    /// * `data` - A slice of `(Asteroid, Vec<Approach>)` tuples to persist.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction fails. On failure, all changes are rolled back.
    #[allow(clippy::too_many_lines)]
    pub async fn upsert_asteroids_and_approaches(
        pool: &PgPool,
        data: &[(Asteroid, Vec<Approach>)],
    ) -> Result<UpsertStats, sqlx::Error> {
        let mut total_stats = UpsertStats::default();

        for chunk in data.chunks(1000) {
            let mut ids: Vec<Uuid> = Vec::new();
            let mut neo_reference_ids: Vec<String> = Vec::new();
            let mut names: Vec<String> = Vec::new();
            let mut absolute_magnitudes: Vec<f64> = Vec::new();
            let mut diameter_mins: Vec<f64> = Vec::new();
            let mut diameter_maxs: Vec<f64> = Vec::new();
            let mut is_hazardous: Vec<bool> = Vec::new();
            let mut is_sentry: Vec<bool> = Vec::new();
            let mut jpl_urls: Vec<String> = Vec::new();
            let mut created_ats: Vec<DateTime<Utc>> = Vec::new();
            let mut updated_ats: Vec<DateTime<Utc>> = Vec::new();

            let mut a_ids: Vec<Uuid> = Vec::new();
            let mut a_asteroid_ids: Vec<Uuid> = Vec::new();
            let mut a_close_approach_dates: Vec<NaiveDate> = Vec::new();
            let mut a_epoch_dates: Vec<i64> = Vec::new();
            let mut a_vel_km_s: Vec<f64> = Vec::new();
            let mut a_vel_km_h: Vec<f64> = Vec::new();
            let mut a_miss_km: Vec<f64> = Vec::new();
            let mut a_miss_au: Vec<f64> = Vec::new();
            let mut a_miss_lunar: Vec<f64> = Vec::new();
            let mut a_orbiting_bodies: Vec<String> = Vec::new();
            let mut a_hazard_classifications: Vec<String> = Vec::new();
            let mut a_created_ats: Vec<DateTime<Utc>> = Vec::new();

            for (asteroid, approaches) in chunk {
                ids.push(asteroid.id);
                neo_reference_ids.push(asteroid.neo_reference_id.clone());
                names.push(asteroid.name.clone());
                absolute_magnitudes.push(asteroid.absolute_magnitude);
                diameter_mins.push(asteroid.estimated_diameter_min_km);
                diameter_maxs.push(asteroid.estimated_diameter_max_km);
                is_hazardous.push(asteroid.is_potentially_hazardous);
                is_sentry.push(asteroid.is_sentry_object);
                jpl_urls.push(asteroid.nasa_jpl_url.clone());
                created_ats.push(asteroid.created_at);
                updated_ats.push(asteroid.updated_at);

                for approach in approaches {
                    a_ids.push(approach.id);
                    a_asteroid_ids.push(approach.asteroid_id);
                    a_close_approach_dates.push(approach.close_approach_date);
                    a_epoch_dates.push(approach.epoch_date_close_approach);
                    a_vel_km_s.push(approach.velocity_km_per_s);
                    a_vel_km_h.push(approach.velocity_km_per_h);
                    a_miss_km.push(approach.miss_distance_km);
                    a_miss_au.push(approach.miss_distance_astronomical);
                    a_miss_lunar.push(approach.miss_distance_lunar);
                    a_orbiting_bodies.push(approach.orbiting_body.clone());
                    a_hazard_classifications.push(approach.hazard_classification.to_string());
                    a_created_ats.push(approach.created_at);
                }
            }

            let mut tx = pool.begin().await?;

            sqlx::query(
                r#"
                INSERT INTO asteroids (
                    id, neo_reference_id, name, absolute_magnitude,
                    estimated_diameter_min_km, estimated_diameter_max_km,
                    is_potentially_hazardous, is_sentry_object,
                    nasa_jpl_url, created_at, updated_at
                )
                SELECT * FROM UNNEST(
                    $1::uuid[], $2::text[], $3::text[], $4::float8[],
                    $5::float8[], $6::float8[], $7::bool[], $8::bool[],
                    $9::text[], $10::timestamptz[], $11::timestamptz[]
                )
                ON CONFLICT (neo_reference_id) DO UPDATE SET
                    name = EXCLUDED.name,
                    absolute_magnitude = EXCLUDED.absolute_magnitude,
                    estimated_diameter_min_km = EXCLUDED.estimated_diameter_min_km,
                    estimated_diameter_max_km = EXCLUDED.estimated_diameter_max_km,
                    is_potentially_hazardous = EXCLUDED.is_potentially_hazardous,
                    is_sentry_object = EXCLUDED.is_sentry_object,
                    nasa_jpl_url = EXCLUDED.nasa_jpl_url,
                    updated_at = EXCLUDED.updated_at
                "#,
            )
            .bind(&ids)
            .bind(&neo_reference_ids)
            .bind(&names)
            .bind(&absolute_magnitudes)
            .bind(&diameter_mins)
            .bind(&diameter_maxs)
            .bind(&is_hazardous)
            .bind(&is_sentry)
            .bind(&jpl_urls)
            .bind(&created_ats)
            .bind(&updated_ats)
            .execute(&mut *tx)
            .await?;

            let approaches_result = sqlx::query(
                r#"
                INSERT INTO approaches (
                    id, asteroid_id, close_approach_date, epoch_date_close_approach,
                    velocity_km_per_s, velocity_km_per_h,
                    miss_distance_km, miss_distance_astronomical, miss_distance_lunar,
                    orbiting_body, hazard_classification, created_at
                )
                SELECT * FROM UNNEST(
                    $1::uuid[], $2::uuid[], $3::date[], $4::int8[],
                    $5::float8[], $6::float8[], $7::float8[], $8::float8[], $9::float8[],
                    $10::text[], $11::text[], $12::timestamptz[]
                )
                ON CONFLICT (asteroid_id, epoch_date_close_approach) DO NOTHING
                "#,
            )
            .bind(&a_ids)
            .bind(&a_asteroid_ids)
            .bind(&a_close_approach_dates)
            .bind(&a_epoch_dates)
            .bind(&a_vel_km_s)
            .bind(&a_vel_km_h)
            .bind(&a_miss_km)
            .bind(&a_miss_au)
            .bind(&a_miss_lunar)
            .bind(&a_orbiting_bodies)
            .bind(&a_hazard_classifications)
            .bind(&a_created_ats)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;

            total_stats.asteroids_inserted += ids.len() as u64;
            total_stats.approaches_inserted += approaches_result.rows_affected();
            total_stats.approaches_skipped +=
                a_ids.len() as u64 - approaches_result.rows_affected();

            debug!(
                asteroids = ids.len(),
                approaches_inserted = approaches_result.rows_affected(),
                approaches_skipped = a_ids.len() as u64 - approaches_result.rows_affected(),
                "Batch committed successfully"
            );
        }

        Ok(total_stats)
    }

    /// Checks if a source file has already been processed by querying the `etl_events` table.
    ///
    /// # Returns
    ///
    /// `true` if the source file has a completed event record, `false` otherwise.
    pub async fn is_file_processed(pool: &PgPool, source_file: &str) -> Result<bool, sqlx::Error> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM etl_events WHERE source_file = $1 AND status = 'completed'",
        )
        .bind(source_file)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|(count,)| count > 0).unwrap_or(false))
    }

    /// Records the start of an ETL event for tracking and idempotency.
    ///
    /// This creates a new entry in `etl_events` with 'running' status. If an
    /// event for the same `source_file` already exists, it is reset to 'running'.
    pub async fn record_etl_event_start(
        pool: &PgPool,
        event_id: Uuid,
        source_file: &str,
        started_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO etl_events (id, source_file, started_at, status)
            VALUES ($1, $2, $3, 'running'::text)
            ON CONFLICT (source_file) DO UPDATE SET
                started_at = EXCLUDED.started_at,
                status = 'running'::text,
                error_message = NULL
            "#,
        )
        .bind(event_id)
        .bind(source_file)
        .bind(started_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Marks an ETL event as completed with processing statistics.
    pub async fn record_etl_event_complete(
        pool: &PgPool,
        event_id: Uuid,
        asteroids_processed: i32,
        approaches_processed: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE etl_events
            SET completed_at = NOW(),
                status = 'completed'::text,
                asteroids_processed = $2,
                approaches_processed = $3
            WHERE id = $1
            "#,
        )
        .bind(event_id)
        .bind(asteroids_processed)
        .bind(approaches_processed)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Marks an ETL event as failed with an error message.
    pub async fn record_etl_event_failed(
        pool: &PgPool,
        event_id: Uuid,
        error_message: &str,
    ) -> Result<(), sqlx::Error> {
        warn!(event_id = %event_id, error = error_message, "Recording ETL event failure");

        sqlx::query(
            r#"
            UPDATE etl_events
            SET completed_at = NOW(),
                status = 'failed'::text,
                error_message = $2
            WHERE id = $1
            "#,
        )
        .bind(event_id)
        .bind(error_message)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Upserts a single batch of records.
    ///
    /// This is a helper method for streaming operations.
    #[allow(clippy::too_many_lines)]
    pub async fn upsert_batch(
        pool: &PgPool,
        data: Vec<(Asteroid, Vec<Approach>)>,
    ) -> Result<UpsertStats, sqlx::Error> {
        let mut ids: Vec<Uuid> = Vec::new();
        let mut neo_reference_ids: Vec<String> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        let mut absolute_magnitudes: Vec<f64> = Vec::new();
        let mut diameter_mins: Vec<f64> = Vec::new();
        let mut diameter_maxs: Vec<f64> = Vec::new();
        let mut is_hazardous: Vec<bool> = Vec::new();
        let mut is_sentry: Vec<bool> = Vec::new();
        let mut jpl_urls: Vec<String> = Vec::new();
        let mut created_ats: Vec<DateTime<Utc>> = Vec::new();
        let mut updated_ats: Vec<DateTime<Utc>> = Vec::new();

        let mut a_ids: Vec<Uuid> = Vec::new();
        let mut a_asteroid_ids: Vec<Uuid> = Vec::new();
        let mut a_close_approach_dates: Vec<NaiveDate> = Vec::new();
        let mut a_epoch_dates: Vec<i64> = Vec::new();
        let mut a_vel_km_s: Vec<f64> = Vec::new();
        let mut a_vel_km_h: Vec<f64> = Vec::new();
        let mut a_miss_km: Vec<f64> = Vec::new();
        let mut a_miss_au: Vec<f64> = Vec::new();
        let mut a_miss_lunar: Vec<f64> = Vec::new();
        let mut a_orbiting_bodies: Vec<String> = Vec::new();
        let mut a_hazard_classifications: Vec<String> = Vec::new();
        let mut a_created_ats: Vec<DateTime<Utc>> = Vec::new();

        // Keyed by approach id, so we can tell which of the newly-*inserted*
        // rows (per the `RETURNING id` below) are hazardous enough to
        // publish, without a second pass over `data`.
        let mut approach_meta: HashMap<Uuid, (String, HazardClassification)> = HashMap::new();

        for (asteroid, approaches) in &data {
            ids.push(asteroid.id);
            neo_reference_ids.push(asteroid.neo_reference_id.clone());
            names.push(asteroid.name.clone());
            absolute_magnitudes.push(asteroid.absolute_magnitude);
            diameter_mins.push(asteroid.estimated_diameter_min_km);
            diameter_maxs.push(asteroid.estimated_diameter_max_km);
            is_hazardous.push(asteroid.is_potentially_hazardous);
            is_sentry.push(asteroid.is_sentry_object);
            jpl_urls.push(asteroid.nasa_jpl_url.clone());
            created_ats.push(asteroid.created_at);
            updated_ats.push(asteroid.updated_at);

            for approach in approaches {
                a_ids.push(approach.id);
                a_asteroid_ids.push(approach.asteroid_id);
                a_close_approach_dates.push(approach.close_approach_date);
                a_epoch_dates.push(approach.epoch_date_close_approach);
                a_vel_km_s.push(approach.velocity_km_per_s);
                a_vel_km_h.push(approach.velocity_km_per_h);
                a_miss_km.push(approach.miss_distance_km);
                a_miss_au.push(approach.miss_distance_astronomical);
                a_miss_lunar.push(approach.miss_distance_lunar);
                a_orbiting_bodies.push(approach.orbiting_body.clone());
                a_hazard_classifications.push(approach.hazard_classification.to_string());
                a_created_ats.push(approach.created_at);

                approach_meta.insert(
                    approach.id,
                    (
                        asteroid.name.clone(),
                        approach.hazard_classification.clone(),
                    ),
                );
            }
        }

        let mut tx = pool.begin().await?;

        let asteroids_result = sqlx::query(
            r#"
            INSERT INTO asteroids (
                id, neo_reference_id, name, absolute_magnitude,
                estimated_diameter_min_km, estimated_diameter_max_km,
                is_potentially_hazardous, is_sentry_object,
                nasa_jpl_url, created_at, updated_at
            )
            SELECT * FROM UNNEST(
                $1::uuid[], $2::text[], $3::text[], $4::float8[],
                $5::float8[], $6::float8[], $7::bool[], $8::bool[],
                $9::text[], $10::timestamptz[], $11::timestamptz[]
            )
            ON CONFLICT (neo_reference_id) DO UPDATE SET
                name = EXCLUDED.name,
                absolute_magnitude = EXCLUDED.absolute_magnitude,
                estimated_diameter_min_km = EXCLUDED.estimated_diameter_min_km,
                estimated_diameter_max_km = EXCLUDED.estimated_diameter_max_km,
                is_potentially_hazardous = EXCLUDED.is_potentially_hazardous,
                is_sentry_object = EXCLUDED.is_sentry_object,
                nasa_jpl_url = EXCLUDED.nasa_jpl_url,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(&ids)
        .bind(&neo_reference_ids)
        .bind(&names)
        .bind(&absolute_magnitudes)
        .bind(&diameter_mins)
        .bind(&diameter_maxs)
        .bind(&is_hazardous)
        .bind(&is_sentry)
        .bind(&jpl_urls)
        .bind(&created_ats)
        .bind(&updated_ats)
        .execute(&mut *tx)
        .await?;

        let inserted_approach_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            INSERT INTO approaches (
                id, asteroid_id, close_approach_date, epoch_date_close_approach,
                velocity_km_per_s, velocity_km_per_h,
                miss_distance_km, miss_distance_astronomical, miss_distance_lunar,
                orbiting_body, hazard_classification, created_at
            )
            SELECT * FROM UNNEST(
                $1::uuid[], $2::uuid[], $3::date[], $4::int8[],
                $5::float8[], $6::float8[], $7::float8[], $8::float8[], $9::float8[],
                $10::text[], $11::text[], $12::timestamptz[]
            )
            ON CONFLICT (asteroid_id, epoch_date_close_approach) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(&a_ids)
        .bind(&a_asteroid_ids)
        .bind(&a_close_approach_dates)
        .bind(&a_epoch_dates)
        .bind(&a_vel_km_s)
        .bind(&a_vel_km_h)
        .bind(&a_miss_km)
        .bind(&a_miss_au)
        .bind(&a_miss_lunar)
        .bind(&a_orbiting_bodies)
        .bind(&a_hazard_classifications)
        .bind(&a_created_ats)
        .fetch_all(&mut *tx)
        .await?;

        tx.commit().await?;

        let asteroids_inserted = asteroids_result.rows_affected();
        let asteroids_updated = ids.len() as u64 - asteroids_inserted;
        let approaches_inserted = inserted_approach_ids.len() as u64;

        let now = Utc::now();
        let new_hazard_events = inserted_approach_ids
            .into_iter()
            .filter_map(|approach_id| {
                let (asteroid_name, hazard_classification) = approach_meta.remove(&approach_id)?;
                matches!(
                    hazard_classification,
                    HazardClassification::Critical | HazardClassification::High
                )
                .then_some(HazardEvent {
                    approach_id,
                    asteroid_name,
                    hazard_classification,
                    timestamp: now,
                })
            })
            .collect();

        Ok(UpsertStats {
            asteroids_inserted,
            asteroids_updated,
            approaches_inserted,
            approaches_skipped: a_ids.len() as u64 - approaches_inserted,
            new_hazard_events,
        })
    }
}
