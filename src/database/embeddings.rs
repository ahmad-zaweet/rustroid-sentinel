//! Persistence for `asteroid_embeddings` (M5 — pgvector similarity).
//!
//! Read side (feature extraction for the `vectorize` CLI job) and write side
//! (batch upsert of computed vectors) both live here; the actual embedding
//! math is in [`crate::transform::embedding`], kept pure and DB-free.

use pgvector::Vector;
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use crate::transform::embedding::AsteroidFeatures;

/// One asteroid's raw feature inputs, as read from the DB.
#[derive(Debug, sqlx::FromRow)]
struct FeatureRow {
    id: Uuid,
    absolute_magnitude: f64,
    estimated_diameter_avg_km: f64,
    is_potentially_hazardous: bool,
    is_sentry_object: bool,
    torino_scale: Option<i16>,
    palermo_scale: Option<f64>,
    eccentricity: Option<f64>,
    semi_major_axis_au: Option<f64>,
    inclination_deg: Option<f64>,
    ascending_node_deg: Option<f64>,
    perihelion_arg_deg: Option<f64>,
    mean_anomaly_deg: Option<f64>,
    orbital_period_days: Option<f64>,
    albedo: Option<f64>,
    velocity_km_per_s: Option<f64>,
    miss_distance_astronomical: Option<f64>,
}

impl From<FeatureRow> for AsteroidFeatures {
    fn from(row: FeatureRow) -> Self {
        AsteroidFeatures {
            absolute_magnitude: row.absolute_magnitude,
            estimated_diameter_avg_km: row.estimated_diameter_avg_km,
            is_potentially_hazardous: row.is_potentially_hazardous,
            is_sentry_object: row.is_sentry_object,
            torino_scale: row.torino_scale,
            palermo_scale: row.palermo_scale,
            eccentricity: row.eccentricity,
            semi_major_axis_au: row.semi_major_axis_au,
            inclination_deg: row.inclination_deg,
            ascending_node_deg: row.ascending_node_deg,
            perihelion_arg_deg: row.perihelion_arg_deg,
            mean_anomaly_deg: row.mean_anomaly_deg,
            orbital_period_days: row.orbital_period_days,
            albedo: row.albedo,
            velocity_km_per_s: row.velocity_km_per_s,
            miss_distance_astronomical: row.miss_distance_astronomical,
        }
    }
}

/// Number of upserted rows flushed to the DB per transaction, so a full
/// reindex never holds the whole candidate set as pending writes.
pub const UPSERT_FLUSH_SIZE: usize = 500;

/// Read/write access to `asteroid_embeddings`.
pub struct EmbeddingRepository;

impl EmbeddingRepository {
    /// Fetches feature inputs for every asteroid. Unlike `orbits`/`sentry`,
    /// this is pure computation over already-stored data with no external
    /// API call, so every asteroid is always a candidate — there's no
    /// staleness tracking to bound the query by.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn fetch_feature_rows(
        pool: &PgPool,
    ) -> Result<Vec<(Uuid, AsteroidFeatures)>, sqlx::Error> {
        info!("Fetching asteroid feature rows for embedding computation");

        let rows: Vec<FeatureRow> = sqlx::query_as(
            r#"
            WITH closest_approach AS (
                SELECT DISTINCT ON (asteroid_id)
                    asteroid_id, velocity_km_per_s, miss_distance_astronomical
                FROM approaches
                ORDER BY asteroid_id, miss_distance_km ASC
            )
            SELECT
                ast.id,
                ast.absolute_magnitude,
                ast.estimated_diameter_avg_km,
                ast.is_potentially_hazardous,
                ast.is_sentry_object,
                ast.torino_scale,
                ast.palermo_scale,
                o.eccentricity,
                o.semi_major_axis_au,
                o.inclination_deg,
                o.ascending_node_deg,
                o.perihelion_arg_deg,
                o.mean_anomaly_deg,
                o.orbital_period_days,
                o.albedo,
                ca.velocity_km_per_s,
                ca.miss_distance_astronomical
            FROM asteroids ast
            LEFT JOIN asteroid_orbits o ON o.asteroid_id = ast.id
            LEFT JOIN closest_approach ca ON ca.asteroid_id = ast.id
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|row| (row.id, row.into())).collect())
    }

    /// Upserts a batch of computed embeddings, flushed in chunks of
    /// [`UPSERT_FLUSH_SIZE`] within their own transaction.
    ///
    /// # Errors
    ///
    /// Returns an error if any transaction fails to commit.
    pub async fn upsert_embeddings(
        pool: &PgPool,
        rows: &[(Uuid, Vector)],
    ) -> Result<u64, sqlx::Error> {
        let mut updated = 0u64;

        for chunk in rows.chunks(UPSERT_FLUSH_SIZE) {
            let mut builder = sqlx::QueryBuilder::new(
                "INSERT INTO asteroid_embeddings (asteroid_id, embedding, computed_at) ",
            );

            builder.push_values(chunk, |mut b, (asteroid_id, embedding)| {
                b.push_bind(asteroid_id).push_bind(embedding).push("NOW()");
            });

            builder.push(
                " ON CONFLICT (asteroid_id) DO UPDATE SET embedding = EXCLUDED.embedding, computed_at = NOW()",
            );

            builder.build().execute(pool).await?;
            updated += chunk.len() as u64;
        }

        Ok(updated)
    }
}
