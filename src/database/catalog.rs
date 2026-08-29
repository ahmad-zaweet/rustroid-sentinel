//! Read-only queries backing the asteroid catalog (`GET /asteroids`,
//! `GET /asteroids/{neo_reference_id}`). Keyset-paginated, never `OFFSET`.

use crate::api::cursor::{CatalogCursor, CursorValue};
use crate::api::types::{
    ApproachRecord, AsteroidCatalogRecord, AsteroidDetailRecord, CatalogSortKey, SortDir,
};
use chrono::NaiveDate;
use sqlx::PgPool;
use sqlx::Postgres;
use sqlx::QueryBuilder;
use tracing::info;
use uuid::Uuid;

/// Number of approach-history rows returned on the detail page.
const APPROACH_HISTORY_LIMIT: i64 = 50;

/// Filter/sort/pagination parameters for [`CatalogRepository::list`].
///
/// Deliberately separate from [`crate::api::types::CatalogQuery`]: this
/// struct carries the already-decoded cursor and borrowed string filters,
/// so the handler owns the (fallible) decoding of client input and this
/// layer only ever sees valid, typed values.
#[derive(Debug, Default)]
pub struct CatalogListParams<'a> {
    /// Decoded position to resume from; `None` starts at the first page.
    pub cursor: Option<CatalogCursor>,
    /// Sort key; see [`CatalogSortKey`].
    pub sort: CatalogSortKey,
    /// Sort direction.
    pub sort_dir: SortDir,
    /// Case-insensitive substring match against `asteroids.name`.
    pub name: Option<&'a str>,
    /// Filter to potentially-hazardous asteroids only.
    pub is_potentially_hazardous: Option<bool>,
    /// Filter to currently-flagged JPL Sentry objects only.
    pub is_sentry_object: Option<bool>,
    /// Asteroid must have at least one approach on/after this date.
    pub start_date: Option<NaiveDate>,
    /// Asteroid must have at least one approach on/before this date.
    pub end_date: Option<NaiveDate>,
    /// Minimum average estimated diameter, in kilometers (inclusive).
    pub min_diameter_km: Option<f64>,
    /// Maximum average estimated diameter, in kilometers (inclusive).
    pub max_diameter_km: Option<f64>,
    /// Orbit classification filter (e.g. "Aten").
    pub orbit_class: Option<&'a str>,
    /// Spectral classification filter (e.g. "Sq").
    pub spectral_class: Option<&'a str>,
    /// Minimum Torino Scale (inclusive).
    pub min_torino_scale: Option<i16>,
    /// Minimum cumulative Palermo Scale (inclusive).
    pub min_palermo_scale: Option<f64>,
    /// Page size. The caller (handler) is responsible for clamping this to
    /// a sane bound before it reaches the query.
    pub limit: u32,
}

/// Appends ` ORDER BY <col> <dir> [NULLS LAST], ast.id <dir>` for the given
/// sort key/direction. Nullable sort columns always sort `NULLS LAST`
/// regardless of direction, so a client toggling direction doesn't cause
/// null rows to jump from the end to the start of the page.
fn push_order_by(query: &mut QueryBuilder<Postgres>, sort: CatalogSortKey, dir: SortDir) {
    let (column, nullable) = match sort {
        CatalogSortKey::ApproachActivity => ("aa.latest_approach_date", true),
        CatalogSortKey::Name => ("ast.name", false),
        CatalogSortKey::Diameter => ("ast.estimated_diameter_avg_km", false),
        CatalogSortKey::Torino => ("ast.torino_scale", true),
        CatalogSortKey::Palermo => ("ast.palermo_scale", true),
    };
    let dir_sql = match dir {
        SortDir::Asc => "ASC",
        SortDir::Desc => "DESC",
    };

    query.push(format!(" ORDER BY {column} {dir_sql}"));
    if nullable {
        query.push(" NULLS LAST");
    }
    query.push(format!(", ast.id {dir_sql}"));
}

/// Appends the keyset `WHERE` predicate that resumes the ordering built by
/// [`push_order_by`] right after `cursor`.
fn push_keyset_predicate(query: &mut QueryBuilder<Postgres>, cursor: &CatalogCursor) {
    match &cursor.value {
        CursorValue::Date(value) => {
            push_nullable_keyset(
                query,
                "aa.latest_approach_date",
                cursor.sort_dir,
                *value,
                cursor.id,
            );
        }
        CursorValue::Torino(value) => {
            push_nullable_keyset(
                query,
                "ast.torino_scale",
                cursor.sort_dir,
                *value,
                cursor.id,
            );
        }
        CursorValue::Palermo(value) => {
            push_nullable_keyset(
                query,
                "ast.palermo_scale",
                cursor.sort_dir,
                *value,
                cursor.id,
            );
        }
        CursorValue::Text(value) => {
            push_not_null_keyset(query, "ast.name", cursor.sort_dir, value.clone(), cursor.id);
        }
        CursorValue::Diameter(value) => {
            push_not_null_keyset(
                query,
                "ast.estimated_diameter_avg_km",
                cursor.sort_dir,
                *value,
                cursor.id,
            );
        }
    }
}

/// Keyset predicate for a nullable sort column, `NULLS LAST` in both
/// directions: a non-null cursor value also has to admit the `NULL` tail (it
/// always comes after every non-null value); a `None` cursor value means
/// we're already inside that tail, and can only keep tie-breaking on `id`.
fn push_nullable_keyset<'t, T>(
    query: &mut QueryBuilder<Postgres>,
    column: &str,
    dir: SortDir,
    value: Option<T>,
    id: Uuid,
) where
    T: Clone + sqlx::Encode<'t, Postgres> + sqlx::Type<Postgres>,
{
    let cmp = match dir {
        SortDir::Asc => ">",
        SortDir::Desc => "<",
    };
    match value {
        Some(v) => {
            query.push(format!(" AND ({column} {cmp} "));
            query.push_bind(v.clone());
            query.push(format!(" OR ({column} = "));
            query.push_bind(v);
            query.push(format!(" AND ast.id {cmp} "));
            query.push_bind(id);
            query.push(format!(") OR {column} IS NULL)"));
        }
        None => {
            query.push(format!(" AND {column} IS NULL AND ast.id {cmp} "));
            query.push_bind(id);
        }
    }
}

/// Keyset predicate for a non-nullable sort column.
fn push_not_null_keyset<'q, T>(
    query: &mut QueryBuilder<Postgres>,
    column: &str,
    dir: SortDir,
    value: T,
    id: Uuid,
) where
    T: 'q + Clone + Send + sqlx::Encode<'q, Postgres> + sqlx::Type<Postgres>,
{
    let cmp = match dir {
        SortDir::Asc => ">",
        SortDir::Desc => "<",
    };
    query.push(format!(" AND ({column} {cmp} "));
    query.push_bind(value.clone());
    query.push(format!(" OR ({column} = "));
    query.push_bind(value);
    query.push(format!(" AND ast.id {cmp} "));
    query.push_bind(id);
    query.push(")");
}

/// Read-only repository backing the asteroid catalog.
pub struct CatalogRepository;

impl CatalogRepository {
    /// Fetches one page of the asteroid catalog listing.
    ///
    /// Sorted by `params.sort`/`params.sort_dir`, nullable sort columns
    /// always `NULLS LAST` regardless of direction, tied by `id` in the same
    /// direction as the primary sort. Fetches `limit + 1` rows so the caller
    /// can tell whether another page exists without a separate `COUNT(*)`;
    /// the returned `Vec` is trimmed back to `limit`.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn list(
        pool: &PgPool,
        params: CatalogListParams<'_>,
    ) -> Result<(Vec<AsteroidCatalogRecord>, bool), sqlx::Error> {
        info!(limit = params.limit, "Fetching asteroid catalog page");

        let mut query: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            r#"
            WITH approach_activity AS (
                SELECT asteroid_id, MAX(close_approach_date) AS latest_approach_date
                FROM approaches
                GROUP BY asteroid_id
            )
            SELECT
                ast.id,
                ast.neo_reference_id,
                ast.name,
                ast.estimated_diameter_avg_km,
                ast.is_potentially_hazardous,
                ast.is_sentry_object,
                ast.torino_scale,
                ast.palermo_scale,
                o.orbit_class,
                o.spectral_class,
                aa.latest_approach_date
            FROM asteroids ast
            LEFT JOIN approach_activity aa ON aa.asteroid_id = ast.id
            LEFT JOIN asteroid_orbits o ON o.asteroid_id = ast.id
            WHERE 1=1
            "#,
        );

        if let Some(name) = params.name {
            query.push(" AND ast.name ILIKE ");
            query.push_bind(format!("%{name}%"));
        }
        if let Some(pha) = params.is_potentially_hazardous {
            query.push(" AND ast.is_potentially_hazardous = ");
            query.push_bind(pha);
        }
        if let Some(sentry) = params.is_sentry_object {
            query.push(" AND ast.is_sentry_object = ");
            query.push_bind(sentry);
        }
        if params.start_date.is_some() || params.end_date.is_some() {
            query.push(" AND EXISTS (SELECT 1 FROM approaches ap WHERE ap.asteroid_id = ast.id");
            if let Some(start) = params.start_date {
                query.push(" AND ap.close_approach_date >= ");
                query.push_bind(start);
            }
            if let Some(end) = params.end_date {
                query.push(" AND ap.close_approach_date <= ");
                query.push_bind(end);
            }
            query.push(")");
        }
        if let Some(min_d) = params.min_diameter_km {
            query.push(" AND ast.estimated_diameter_avg_km >= ");
            query.push_bind(min_d);
        }
        if let Some(max_d) = params.max_diameter_km {
            query.push(" AND ast.estimated_diameter_avg_km <= ");
            query.push_bind(max_d);
        }
        if let Some(orbit_class) = params.orbit_class {
            query.push(" AND o.orbit_class = ");
            query.push_bind(orbit_class);
        }
        if let Some(spectral_class) = params.spectral_class {
            query.push(" AND o.spectral_class = ");
            query.push_bind(spectral_class);
        }
        if let Some(min_torino) = params.min_torino_scale {
            query.push(" AND ast.torino_scale >= ");
            query.push_bind(min_torino);
        }
        if let Some(min_palermo) = params.min_palermo_scale {
            query.push(" AND ast.palermo_scale >= ");
            query.push_bind(min_palermo);
        }

        // Keyset predicate: rows strictly after `cursor` in the current
        // sort's ordering. A non-null cursor value on a nullable column also
        // has to admit the NULL tail (it always comes after every non-null
        // value, in either direction); a null cursor value means we're
        // already inside that tail, and can only keep tie-breaking on `id`.
        if let Some(cursor) = params.cursor {
            push_keyset_predicate(&mut query, &cursor);
        }

        push_order_by(&mut query, params.sort, params.sort_dir);
        query.push(" LIMIT ");
        query.push_bind(i64::from(params.limit) + 1);

        let mut rows: Vec<AsteroidCatalogRecord> = query.build_query_as().fetch_all(pool).await?;

        let has_more = rows.len() > params.limit as usize;
        rows.truncate(params.limit as usize);

        Ok((rows, has_more))
    }

    /// Fetches the detail view for a single asteroid, including its
    /// orbital elements and close-approach history.
    ///
    /// Returns `Ok(None)` if no asteroid matches `neo_reference_id`.
    ///
    /// # Errors
    ///
    /// Returns an error if any query fails.
    pub async fn get_detail(
        pool: &PgPool,
        neo_reference_id: &str,
    ) -> Result<Option<AsteroidDetailRecord>, sqlx::Error> {
        info!(neo_reference_id, "Fetching asteroid detail");

        let summary: Option<AsteroidCatalogRecord> = sqlx::query_as(
            r#"
            WITH approach_activity AS (
                SELECT asteroid_id, MAX(close_approach_date) AS latest_approach_date
                FROM approaches
                GROUP BY asteroid_id
            )
            SELECT
                ast.id,
                ast.neo_reference_id,
                ast.name,
                ast.estimated_diameter_avg_km,
                ast.is_potentially_hazardous,
                ast.is_sentry_object,
                ast.torino_scale,
                ast.palermo_scale,
                o.orbit_class,
                o.spectral_class,
                aa.latest_approach_date
            FROM asteroids ast
            LEFT JOIN approach_activity aa ON aa.asteroid_id = ast.id
            LEFT JOIN asteroid_orbits o ON o.asteroid_id = ast.id
            WHERE ast.neo_reference_id = $1
            "#,
        )
        .bind(neo_reference_id)
        .fetch_optional(pool)
        .await?;

        let Some(summary) = summary else {
            return Ok(None);
        };

        let orbit_extra: (Option<f64>, Option<f64>, Option<f64>, Option<f64>, String) =
            sqlx::query_as(
                r#"
                SELECT o.eccentricity, o.semi_major_axis_au, o.inclination_deg, o.albedo,
                       ast.nasa_jpl_url
                FROM asteroids ast
                LEFT JOIN asteroid_orbits o ON o.asteroid_id = ast.id
                WHERE ast.id = $1
                "#,
            )
            .bind(summary.id)
            .fetch_one(pool)
            .await?;

        let approaches: Vec<ApproachRecord> = sqlx::query_as(
            r#"
            SELECT
                a.id,
                ast.name as asteroid_name,
                ast.neo_reference_id,
                a.close_approach_date,
                a.velocity_km_per_h,
                a.miss_distance_km,
                a.hazard_classification,
                ast.is_potentially_hazardous,
                ast.estimated_diameter_avg_km,
                ast.torino_scale,
                ast.palermo_scale
            FROM approaches a
            JOIN asteroids ast ON a.asteroid_id = ast.id
            WHERE ast.id = $1
            ORDER BY a.close_approach_date DESC
            LIMIT $2
            "#,
        )
        .bind(summary.id)
        .bind(APPROACH_HISTORY_LIMIT)
        .fetch_all(pool)
        .await?;

        Ok(Some(AsteroidDetailRecord {
            summary,
            eccentricity: orbit_extra.0,
            semi_major_axis_au: orbit_extra.1,
            inclination_deg: orbit_extra.2,
            albedo: orbit_extra.3,
            nasa_jpl_url: orbit_extra.4,
            approaches,
        }))
    }

    /// Fetches the nearest neighbors of one asteroid's embedding (M5).
    ///
    /// Returns `Ok(None)` if `neo_reference_id` doesn't match an asteroid, or
    /// if it hasn't been vectorized yet (`asteroid_embeddings` has no row for
    /// it) — distinguished from "vectorized but zero neighbors exist", which
    /// returns `Ok(Some(vec![]))`.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn similar(
        pool: &PgPool,
        neo_reference_id: &str,
        limit: i64,
    ) -> Result<Option<Vec<AsteroidCatalogRecord>>, sqlx::Error> {
        info!(neo_reference_id, "Fetching similar asteroids");

        let self_embedding: Option<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT e.asteroid_id
            FROM asteroid_embeddings e
            JOIN asteroids ast ON ast.id = e.asteroid_id
            WHERE ast.neo_reference_id = $1
            "#,
        )
        .bind(neo_reference_id)
        .fetch_optional(pool)
        .await?;

        let Some((asteroid_id,)) = self_embedding else {
            return Ok(None);
        };

        let rows: Vec<AsteroidCatalogRecord> = sqlx::query_as(
            r#"
            WITH approach_activity AS (
                SELECT asteroid_id, MAX(close_approach_date) AS latest_approach_date
                FROM approaches
                GROUP BY asteroid_id
            )
            SELECT
                ast.id,
                ast.neo_reference_id,
                ast.name,
                ast.estimated_diameter_avg_km,
                ast.is_potentially_hazardous,
                ast.is_sentry_object,
                ast.torino_scale,
                ast.palermo_scale,
                o.orbit_class,
                o.spectral_class,
                aa.latest_approach_date
            FROM asteroid_embeddings e
            JOIN asteroid_embeddings self_e ON self_e.asteroid_id = $1
            JOIN asteroids ast ON ast.id = e.asteroid_id
            LEFT JOIN approach_activity aa ON aa.asteroid_id = ast.id
            LEFT JOIN asteroid_orbits o ON o.asteroid_id = ast.id
            WHERE e.asteroid_id != $1
            ORDER BY e.embedding <-> self_e.embedding
            LIMIT $2
            "#,
        )
        .bind(asteroid_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(Some(rows))
    }

    /// Distinct, non-null `orbit_class` and `spectral_class` values present
    /// in `asteroid_orbits` today, each sorted alphabetically. Backs the
    /// catalog page's classification filter dropdowns — since both columns
    /// are free-text (no fixed taxonomy enforced in the DB), this reflects
    /// whatever values ingestion has actually written rather than a
    /// hardcoded list that can drift out of sync.
    pub async fn distinct_classification_values(
        pool: &PgPool,
    ) -> Result<(Vec<String>, Vec<String>), sqlx::Error> {
        let (orbit_classes, spectral_classes): (Option<Vec<String>>, Option<Vec<String>>) =
            sqlx::query_as(
                "SELECT
                    array_agg(DISTINCT orbit_class ORDER BY orbit_class)
                        FILTER (WHERE orbit_class IS NOT NULL),
                    array_agg(DISTINCT spectral_class ORDER BY spectral_class)
                        FILTER (WHERE spectral_class IS NOT NULL)
                 FROM asteroid_orbits",
            )
            .fetch_one(pool)
            .await?;

        Ok((
            orbit_classes.unwrap_or_default(),
            spectral_classes.unwrap_or_default(),
        ))
    }
}
