//! Write-heavy database operations managing the core domain models.
//!
//! Handles UPSERTs of asteroids and approaches in performant batches using `sqlx`.

use crate::events::HazardEvent;
use crate::models::HazardClassification;
use crate::models::approach::Approach;
use crate::models::asteroid::Asteroid;
use crate::nasa::jpl_sbdb::responses::SbdbOrbitSummary;
use chrono::{DateTime, NaiveDate, Utc};
use futures_util::Stream;
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
    /// Ids of asteroids that were newly inserted (not merely updated) by
    /// this batch, so callers can scope follow-up enrichment (Sentry scale,
    /// orbital elements) to just-arrived rows instead of the whole catalog.
    pub newly_inserted_asteroid_ids: Vec<Uuid>,
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

        let asteroid_rows: Vec<(Uuid, bool)> = sqlx::query_as(
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
            RETURNING id, (xmax = 0) AS inserted
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
        .fetch_all(&mut *tx)
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

        let newly_inserted_asteroid_ids: Vec<Uuid> = asteroid_rows
            .iter()
            .filter(|(_, inserted)| *inserted)
            .map(|(id, _)| *id)
            .collect();
        let asteroids_inserted = newly_inserted_asteroid_ids.len() as u64;
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
            newly_inserted_asteroid_ids,
        })
    }

    /// Returns `(id, neo_reference_id)` for asteroids the `sentry` CLI
    /// command should (re-)check against the JPL Sentry API.
    ///
    /// Bounded to `is_sentry_object` rows: NeoWs already tells us, per
    /// object, whether it currently appears on JPL's Sentry Risk List — that
    /// flag *is* current-virtual-impactor membership, so there's no reason
    /// to also sweep the much larger `is_potentially_hazardous` set (PHA
    /// status doesn't imply VI status, and the vast majority of PHAs will
    /// never be on Sentry). A row qualifies if it's never been checked, or
    /// was last checked before `stale_before` (the caller computes this from
    /// `JplSentryConfig::stale_days`, or passes `Utc::now()` to force a full
    /// recheck of every candidate).
    pub async fn asteroids_needing_sentry_check(
        pool: &PgPool,
        stale_before: DateTime<Utc>,
    ) -> Result<Vec<(Uuid, String)>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT id, neo_reference_id FROM asteroids
            WHERE is_sentry_object
              AND (sentry_checked_at IS NULL OR sentry_checked_at < $1)
            ORDER BY neo_reference_id
            "#,
        )
        .bind(stale_before)
        .fetch_all(pool)
        .await
    }

    /// Returns `(id, neo_reference_id)` for exactly the given asteroid ids
    /// that are `is_sentry_object`, with no staleness gate — used to scope
    /// Sentry enrichment to asteroids newly inserted by this pipeline run
    /// (which have no `sentry_checked_at` yet, so every match is eligible by
    /// construction).
    pub async fn sentry_candidates_for_ids(
        pool: &PgPool,
        ids: &[Uuid],
    ) -> Result<Vec<(Uuid, String)>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        sqlx::query_as(
            r#"
            SELECT id, neo_reference_id FROM asteroids
            WHERE is_sentry_object AND id = ANY($1::uuid[])
            ORDER BY neo_reference_id
            "#,
        )
        .bind(ids)
        .fetch_all(pool)
        .await
    }

    /// Bulk-updates `torino_scale`, `palermo_scale`, and `sentry_checked_at`
    /// for a batch of asteroids, without touching any other column. `None`
    /// scale values are written as `NULL` (the object isn't a current
    /// Sentry virtual impactor); `sentry_checked_at` is always stamped so a
    /// permanently-clear asteroid isn't re-queried on every run.
    ///
    /// Returns the number of rows updated.
    pub async fn update_sentry_scales(
        pool: &PgPool,
        updates: &[(Uuid, Option<i16>, Option<f64>)],
    ) -> Result<u64, sqlx::Error> {
        if updates.is_empty() {
            return Ok(0);
        }

        let ids: Vec<Uuid> = updates.iter().map(|(id, _, _)| *id).collect();
        let torino_scales: Vec<Option<i16>> = updates.iter().map(|(_, t, _)| *t).collect();
        let palermo_scales: Vec<Option<f64>> = updates.iter().map(|(_, _, p)| *p).collect();
        let checked_at = Utc::now();

        let result = sqlx::query(
            r#"
            UPDATE asteroids AS a
            SET torino_scale = src.torino_scale,
                palermo_scale = src.palermo_scale,
                sentry_checked_at = $4
            FROM (
                SELECT * FROM UNNEST($1::uuid[], $2::int2[], $3::float8[])
                    AS t(id, torino_scale, palermo_scale)
            ) AS src
            WHERE a.id = src.id
            "#,
        )
        .bind(&ids)
        .bind(&torino_scales)
        .bind(&palermo_scales)
        .bind(checked_at)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Counts asteroids the `orbits` CLI command should (re-)fetch orbital
    /// elements for, without materializing the rows — used only to report
    /// "checked N of TOTAL" progress against
    /// [`stream_asteroids_needing_orbit_check`]'s streamed results.
    ///
    /// Same eligibility rule as the stream: no `asteroid_orbits` row yet, or
    /// `orbit_checked_at` older than `stale_before`.
    pub async fn count_asteroids_needing_orbit_check(
        pool: &PgPool,
        stale_before: DateTime<Utc>,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM (
                (SELECT a.id FROM asteroids a
                    WHERE NOT EXISTS (SELECT 1 FROM asteroid_orbits o WHERE o.asteroid_id = a.id))
                UNION ALL
                (SELECT a.id FROM asteroids a
                    JOIN asteroid_orbits o ON o.asteroid_id = a.id
                    WHERE o.orbit_checked_at < $1)
            ) AS candidates
            "#,
        )
        .bind(stale_before)
        .fetch_one(pool)
        .await
    }

    /// Streams `(id, neo_reference_id)` for asteroids the `orbits` CLI
    /// command should (re-)fetch orbital elements for from JPL's SBDB API,
    /// instead of collecting the full candidate set into memory up front —
    /// the candidate count scales with the whole catalog (every asteroid is
    /// eligible, not just a hazard-flagged subset), so this keeps memory use
    /// independent of catalog size and lets the caller start SBDB lookups as
    /// rows arrive rather than waiting on the full result set.
    ///
    /// Unlike the Sentry check, this sweeps every asteroid, not just
    /// `is_sentry_object` rows — orbital elements are catalog metadata, not
    /// tied to impact-risk status. The two eligibility branches (no orbit row
    /// yet vs. stale orbit row) are written as a `UNION ALL` of two
    /// independently indexable queries rather than a single `LEFT JOIN ...
    /// WHERE x IS NULL OR y < $1`, since the OR-across-a-join form tends to
    /// defeat the `orbit_checked_at` index and fall back to a sequential
    /// scan. The caller computes `stale_before` from
    /// `JplSbdbConfig::stale_days`, or passes `Utc::now()` to force a full
    /// recheck of every candidate.
    pub fn stream_asteroids_needing_orbit_check(
        pool: &PgPool,
        stale_before: DateTime<Utc>,
    ) -> impl Stream<Item = Result<(Uuid, String), sqlx::Error>> + '_ {
        sqlx::query_as(
            r#"
            (SELECT a.id, a.neo_reference_id FROM asteroids a
                WHERE NOT EXISTS (SELECT 1 FROM asteroid_orbits o WHERE o.asteroid_id = a.id))
            UNION ALL
            (SELECT a.id, a.neo_reference_id FROM asteroids a
                JOIN asteroid_orbits o ON o.asteroid_id = a.id
                WHERE o.orbit_checked_at < $1)
            ORDER BY neo_reference_id
            "#,
        )
        .bind(stale_before)
        .fetch(pool)
    }

    /// Returns `(id, neo_reference_id)` for exactly the given asteroid ids,
    /// with no staleness gate — used to scope orbit-elements enrichment to
    /// asteroids newly inserted by this pipeline run (which have no
    /// `orbit_checked_at` yet, so every id is eligible by construction).
    pub async fn orbit_candidates_for_ids(
        pool: &PgPool,
        ids: &[Uuid],
    ) -> Result<Vec<(Uuid, String)>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        sqlx::query_as(
            r#"
            SELECT id, neo_reference_id FROM asteroids
            WHERE id = ANY($1::uuid[])
            ORDER BY neo_reference_id
            "#,
        )
        .bind(ids)
        .fetch_all(pool)
        .await
    }

    /// Bulk-upserts orbital elements into `asteroid_orbits` for a batch of
    /// asteroids, stamping `orbit_checked_at` on every row (including
    /// no-match rows) so a permanently orbit-less asteroid isn't re-queried
    /// on every run.
    ///
    /// Returns the number of rows affected.
    pub async fn upsert_asteroid_orbits(
        pool: &PgPool,
        updates: &[(Uuid, SbdbOrbitSummary)],
    ) -> Result<u64, sqlx::Error> {
        if updates.is_empty() {
            return Ok(0);
        }

        let ids: Vec<Uuid> = updates.iter().map(|(id, _)| *id).collect();
        let eccentricity: Vec<Option<f64>> = updates.iter().map(|(_, s)| s.eccentricity).collect();
        let semi_major_axis_au: Vec<Option<f64>> =
            updates.iter().map(|(_, s)| s.semi_major_axis_au).collect();
        let inclination_deg: Vec<Option<f64>> =
            updates.iter().map(|(_, s)| s.inclination_deg).collect();
        let ascending_node_deg: Vec<Option<f64>> =
            updates.iter().map(|(_, s)| s.ascending_node_deg).collect();
        let perihelion_arg_deg: Vec<Option<f64>> =
            updates.iter().map(|(_, s)| s.perihelion_arg_deg).collect();
        let mean_anomaly_deg: Vec<Option<f64>> =
            updates.iter().map(|(_, s)| s.mean_anomaly_deg).collect();
        let orbital_period_days: Vec<Option<f64>> =
            updates.iter().map(|(_, s)| s.orbital_period_days).collect();
        let orbit_class: Vec<Option<String>> =
            updates.iter().map(|(_, s)| s.orbit_class.clone()).collect();
        let spectral_class: Vec<Option<String>> = updates
            .iter()
            .map(|(_, s)| s.spectral_class.clone())
            .collect();
        let albedo: Vec<Option<f64>> = updates.iter().map(|(_, s)| s.albedo).collect();
        let checked_at = Utc::now();

        let result = sqlx::query(
            r#"
            INSERT INTO asteroid_orbits (
                asteroid_id, eccentricity, semi_major_axis_au, inclination_deg,
                ascending_node_deg, perihelion_arg_deg, mean_anomaly_deg,
                orbital_period_days, orbit_class, spectral_class, albedo,
                orbit_checked_at
            )
            SELECT t.*, $12::timestamptz FROM UNNEST(
                $1::uuid[], $2::float8[], $3::float8[], $4::float8[],
                $5::float8[], $6::float8[], $7::float8[],
                $8::float8[], $9::text[], $10::text[], $11::float8[]
            ) AS t(
                asteroid_id, eccentricity, semi_major_axis_au, inclination_deg,
                ascending_node_deg, perihelion_arg_deg, mean_anomaly_deg,
                orbital_period_days, orbit_class, spectral_class, albedo
            )
            ON CONFLICT (asteroid_id) DO UPDATE SET
                eccentricity = EXCLUDED.eccentricity,
                semi_major_axis_au = EXCLUDED.semi_major_axis_au,
                inclination_deg = EXCLUDED.inclination_deg,
                ascending_node_deg = EXCLUDED.ascending_node_deg,
                perihelion_arg_deg = EXCLUDED.perihelion_arg_deg,
                mean_anomaly_deg = EXCLUDED.mean_anomaly_deg,
                orbital_period_days = EXCLUDED.orbital_period_days,
                orbit_class = EXCLUDED.orbit_class,
                spectral_class = EXCLUDED.spectral_class,
                albedo = EXCLUDED.albedo,
                orbit_checked_at = EXCLUDED.orbit_checked_at
            "#,
        )
        .bind(&ids)
        .bind(&eccentricity)
        .bind(&semi_major_axis_au)
        .bind(&inclination_deg)
        .bind(&ascending_node_deg)
        .bind(&perihelion_arg_deg)
        .bind(&mean_anomaly_deg)
        .bind(&orbital_period_days)
        .bind(&orbit_class)
        .bind(&spectral_class)
        .bind(&albedo)
        .bind(checked_at)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }
}
