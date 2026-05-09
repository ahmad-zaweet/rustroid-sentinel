//! Read-only database operations for compiling dashboard statistics and visualizations.

use crate::api::types::{ApproachRecord, EtlRunRecord, TimePeriod, VelocityDataPoint};
use chrono::NaiveDate;
use sqlx::PgPool;
use tracing::info;

/// Repository for dashboard read operations.
///
/// This repository encapsulates all read queries used by the dashboard API,
/// providing a clean separation between read and write operations.
pub struct DashboardRepository;

/// Dashboard statistics returned from the stats endpoint.
///
/// Provides a high-level overview of the data stored in the system.
#[derive(Debug, Default)]
pub struct DashboardStats {
    /// Total number of unique asteroids in the database.
    pub total_asteroids: i64,
    /// Total number of recorded close approach events.
    pub total_approaches: i64,
    /// Number of asteroids flagged as potentially hazardous.
    pub hazardous_count: i64,
}

impl std::fmt::Display for DashboardStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Asteroids: {} | Approaches: {} | Hazardous: {}",
            self.total_asteroids, self.total_approaches, self.hazardous_count
        )
    }
}

/// Query parameters for fetching paginated approaches.
#[derive(Debug, Default, Clone)]
pub struct ApproachQueryParams<'a> {
    /// Page number (1-indexed).
    pub page: u32,
    /// Number of items per page.
    pub page_size: u32,
    /// Optional start date for filtering.
    pub start_date: Option<NaiveDate>,
    /// Optional end date for filtering.
    pub end_date: Option<NaiveDate>,
    /// Optional hazard classification filter.
    pub hazard_class: Option<&'a str>,
    /// Optional sort column key: "date", "velocity", "distance", "name", "hazard".
    pub sort_by: Option<&'a str>,
    /// Optional sort direction: "asc" or "desc" (default "desc").
    pub sort_dir: Option<&'a str>,
}

impl DashboardRepository {
    /// Gets dashboard statistics including counts of asteroids and approaches.
    ///
    /// # Arguments
    ///
    /// * `pool` - The database connection pool.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the count queries fail.
    pub async fn get_stats(pool: &PgPool) -> Result<DashboardStats, sqlx::Error> {
        info!("Fetching dashboard statistics");

        let total_asteroids = Self::get_asteroid_count(pool).await?;
        let total_approaches = Self::get_approach_count(pool).await?;
        let hazardous_count = Self::get_hazardous_asteroid_count(pool).await?;

        Ok(DashboardStats {
            total_asteroids,
            total_approaches,
            hazardous_count,
        })
    }

    /// Gets total count of asteroids.
    pub async fn get_asteroid_count(pool: &PgPool) -> Result<i64, sqlx::Error> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM asteroids")
            .fetch_one(pool)
            .await?;
        Ok(count.0)
    }

    /// Gets total count of approaches.
    pub async fn get_approach_count(pool: &PgPool) -> Result<i64, sqlx::Error> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM approaches")
            .fetch_one(pool)
            .await?;
        Ok(count.0)
    }

    /// Gets count of potentially hazardous asteroids.
    pub async fn get_hazardous_asteroid_count(pool: &PgPool) -> Result<i64, sqlx::Error> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM asteroids WHERE is_potentially_hazardous = TRUE")
                .fetch_one(pool)
                .await?;
        Ok(count.0)
    }

    /// Gets recent approaches for the dashboard (limited to 50).
    ///
    /// Returns a list of [`ApproachRecord`] objects, ordered by approach date
    /// (descending) and creation timestamp (descending) to ensure stable paging.
    pub async fn get_recent_approaches(pool: &PgPool) -> Result<Vec<ApproachRecord>, sqlx::Error> {
        info!("Fetching recent approaches");

        let approaches: Vec<ApproachRecord> = sqlx::query_as(
            r#"
            SELECT
                a.id,
                ast.name as asteroid_name,
                a.close_approach_date,
                a.velocity_km_per_h,
                a.miss_distance_km,
                a.hazard_classification,
                ast.is_potentially_hazardous
            FROM approaches a
            JOIN asteroids ast ON a.asteroid_id = ast.id
            ORDER BY a.close_approach_date DESC, a.created_at DESC
            LIMIT 50
            "#,
        )
        .fetch_all(pool)
        .await?;
        Ok(approaches)
    }

    /// Gets velocity data for the time-series chart (limited to 100).
    ///
    /// Returns a list of [`VelocityDataPoint`] objects, ordered by approach date
    /// (descending). Used for visualizing approach speeds over time.
    pub async fn get_velocity_data(pool: &PgPool) -> Result<Vec<VelocityDataPoint>, sqlx::Error> {
        info!("Fetching velocity data");

        let data: Vec<VelocityDataPoint> = sqlx::query_as(
            r#"
            SELECT
                a.close_approach_date as date,
                ast.name as asteroid_name,
                a.velocity_km_per_h
            FROM approaches a
            JOIN asteroids ast ON a.asteroid_id = ast.id
            ORDER BY a.close_approach_date DESC
            LIMIT 100
            "#,
        )
        .fetch_all(pool)
        .await?;
        Ok(data)
    }

    /// Gets velocity data with period-based filtering.
    ///
    /// # Arguments
    ///
    /// * `pool` - The database connection pool.
    /// * `period` - Type-safe [`TimePeriod`] enum.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn get_velocity_data_by_period(
        pool: &PgPool,
        period: TimePeriod,
    ) -> Result<Vec<VelocityDataPoint>, sqlx::Error> {
        info!("Fetching velocity data for period: {:?}", period);

        let interval = period.as_sql_interval();

        let data: Vec<VelocityDataPoint> = sqlx::query_as(
            r#"
            SELECT
                a.close_approach_date as date,
                ast.name as asteroid_name,
                a.velocity_km_per_h
            FROM approaches a
            JOIN asteroids ast ON a.asteroid_id = ast.id
            WHERE a.close_approach_date >= NOW() - ($1::INTERVAL)
            ORDER BY a.close_approach_date ASC
            "#,
        )
        .bind(interval)
        .fetch_all(pool)
        .await?;

        Ok(data)
    }

    /// Gets velocity data with custom date range filter.
    ///
    /// # Arguments
    ///
    /// * `pool` - The database connection pool.
    /// * `start_date` - Optional start date for filtering.
    /// * `end_date` - Optional end date for filtering.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub async fn get_velocity_data_with_filter(
        pool: &PgPool,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<VelocityDataPoint>, sqlx::Error> {
        info!(
            "Fetching velocity data from {:?} to {:?}",
            start_date, end_date
        );

        let mut query_builder: sqlx::QueryBuilder<sqlx::Postgres> = sqlx::QueryBuilder::new(
            r#"
            SELECT
                a.close_approach_date as date,
                ast.name as asteroid_name,
                a.velocity_km_per_h
            FROM approaches a
            JOIN asteroids ast ON a.asteroid_id = ast.id
            WHERE 1=1
            "#,
        );

        if let Some(start) = start_date {
            query_builder.push(" AND a.close_approach_date >= ");
            query_builder.push_bind(start);
        }

        if let Some(end) = end_date {
            query_builder.push(" AND a.close_approach_date <= ");
            query_builder.push_bind(end);
        }

        query_builder.push(" ORDER BY a.close_approach_date ASC");

        let data: Vec<VelocityDataPoint> = query_builder.build_query_as().fetch_all(pool).await?;
        Ok(data)
    }

    /// Gets paginated approaches with optional filters.
    ///
    /// # Arguments
    ///
    /// * `pool` - The database connection pool.
    /// * `params` - Query parameters including pagination, filters, and sorting.
    ///
    /// # Returns
    ///
    /// A tuple containing the vector of approaches and the total count.
    ///
    /// # Errors
    ///
    /// Returns an error if any query fails.
    pub async fn get_paginated_approaches(
        pool: &PgPool,
        params: ApproachQueryParams<'_>,
    ) -> Result<(Vec<ApproachRecord>, i64), sqlx::Error> {
        info!(
            "Fetching paginated approaches - page: {}, size: {}, hazard: {:?}",
            params.page, params.page_size, params.hazard_class
        );

        let offset = ((params.page - 1) * params.page_size) as i64;
        let limit = params.page_size as i64;

        let mut count_query: sqlx::QueryBuilder<sqlx::Postgres> = sqlx::QueryBuilder::new(
            r#"
            SELECT COUNT(*)
            FROM approaches a
            JOIN asteroids ast ON a.asteroid_id = ast.id
            WHERE 1=1
            "#,
        );

        let mut data_query: sqlx::QueryBuilder<sqlx::Postgres> = sqlx::QueryBuilder::new(
            r#"
            SELECT
                a.id,
                ast.name as asteroid_name,
                a.close_approach_date,
                a.velocity_km_per_h,
                a.miss_distance_km,
                a.hazard_classification,
                ast.is_potentially_hazardous
            FROM approaches a
            JOIN asteroids ast ON a.asteroid_id = ast.id
            WHERE 1=1
            "#,
        );

        if let Some(start) = params.start_date {
            count_query.push(" AND a.close_approach_date >= ");
            count_query.push_bind(start);
            data_query.push(" AND a.close_approach_date >= ");
            data_query.push_bind(start);
        }

        if let Some(end) = params.end_date {
            let next_day = end
                .checked_add_days(chrono::Days::new(1))
                .expect("date overflow");
            count_query.push(" AND a.close_approach_date < ");
            count_query.push_bind(next_day);
            data_query.push(" AND a.close_approach_date < ");
            data_query.push_bind(next_day);
        }

        if let Some(hazard) = params.hazard_class {
            count_query.push(" AND a.hazard_classification = ");
            count_query.push_bind(hazard);
            data_query.push(" AND a.hazard_classification = ");
            data_query.push_bind(hazard);
        }

        let total: (i64,) = count_query.build_query_as().fetch_one(pool).await?;

        let sort_col = match params.sort_by {
            Some("velocity") => "a.velocity_km_per_h",
            Some("distance") => "a.miss_distance_km",
            Some("name") => "ast.name",
            Some("hazard") => "a.hazard_classification",
            _ => "a.close_approach_date",
        };
        let sort_order = match params.sort_dir {
            Some("asc") => "ASC",
            _ => "DESC",
        };
        data_query.push(format!(
            " ORDER BY {} {}, a.created_at DESC",
            sort_col, sort_order
        ));
        data_query.push(" LIMIT ");
        data_query.push_bind(limit);
        data_query.push(" OFFSET ");
        data_query.push_bind(offset);

        let approaches: Vec<ApproachRecord> = data_query.build_query_as().fetch_all(pool).await?;

        Ok((approaches, total.0))
    }

    /// Gets recent ETL runs (limited to 20).
    ///
    /// Returns ETL runs ordered by start time descending, limited to 20 records.
    pub async fn get_recent_etl_runs(pool: &PgPool) -> Result<Vec<EtlRunRecord>, sqlx::Error> {
        info!("Fetching recent ETL runs");

        let runs: Vec<EtlRunRecord> = sqlx::query_as(
            r#"
            SELECT
                id,
                source_file,
                started_at,
                completed_at,
                status,
                asteroids_processed,
                approaches_processed,
                error_message
            FROM etl_events
            ORDER BY started_at DESC
            LIMIT 20
            "#,
        )
        .fetch_all(pool)
        .await?;
        Ok(runs)
    }

    /// Gets paginated ETL runs.
    ///
    /// # Arguments
    ///
    /// * `pool` - The database connection pool.
    /// * `page` - Page number (1-indexed).
    /// * `page_size` - Number of items per page.
    ///
    /// # Returns
    ///
    /// A tuple containing the vector of ETL runs and the total count.
    ///
    /// # Errors
    ///
    /// Returns an error if any query fails.
    pub async fn get_paginated_etl_runs(
        pool: &PgPool,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<EtlRunRecord>, i64), sqlx::Error> {
        info!(
            "Fetching paginated ETL runs - page: {}, size: {}",
            page, page_size
        );

        let offset = ((page - 1) * page_size) as i64;
        let limit = page_size as i64;

        // Count total ETL runs
        let total: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM etl_events
            "#,
        )
        .fetch_one(pool)
        .await?;

        // Fetch paginated ETL runs
        let runs: Vec<EtlRunRecord> = sqlx::query_as(
            r#"
            SELECT
                id,
                source_file,
                started_at,
                completed_at,
                status,
                asteroids_processed,
                approaches_processed,
                error_message
            FROM etl_events
            ORDER BY started_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok((runs, total.0))
    }

    /// Checks if the database is connected and responsive.
    ///
    /// # Returns
    ///
    /// `true` if the database responds to a simple query, `false` otherwise.
    pub async fn check_connection(pool: &PgPool) -> bool {
        sqlx::query("SELECT 1").fetch_optional(pool).await.is_ok()
    }
}
