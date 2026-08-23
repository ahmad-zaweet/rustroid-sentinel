//! Read-only aggregation queries for the weekly Discord report ([`crate::cli::report`]).

use chrono::NaiveDate;
use sqlx::PgPool;

/// Repository for the weekly report's read queries.
pub struct ReportRepository;

/// A single notable approach (closest, fastest, or largest) called out in the
/// weekly report, alongside its asteroid's name.
#[derive(Debug, sqlx::FromRow)]
pub struct NotableApproach {
    /// Name of the asteroid that made this approach.
    pub asteroid_name: String,
    /// Date the approach occurred.
    pub close_approach_date: NaiveDate,
    /// Miss distance in kilometers.
    pub miss_distance_km: f64,
    /// Relative velocity in km/h.
    pub velocity_km_per_h: f64,
    /// Average estimated diameter of the asteroid, in kilometers.
    pub estimated_diameter_avg_km: f64,
}

/// Aggregated stats over a date range, used to render the weekly Discord report.
#[derive(Debug, Default)]
pub struct WeeklyReportSummary {
    /// Start of the reporting window (inclusive).
    pub start_date: NaiveDate,
    /// End of the reporting window (inclusive).
    pub end_date: NaiveDate,
    /// Total approaches recorded in the window.
    pub total_approaches: i64,
    /// Count of `Critical`-classified approaches.
    pub critical_count: i64,
    /// Count of `High`-classified approaches.
    pub high_count: i64,
    /// Count of `Medium`-classified approaches.
    pub medium_count: i64,
    /// Count of `Low`-classified approaches.
    pub low_count: i64,
    /// Approach with the smallest `miss_distance_km` in the window.
    pub closest_approach: Option<NotableApproach>,
    /// Approach with the largest `velocity_km_per_h` in the window.
    pub fastest_approach: Option<NotableApproach>,
    /// Approach whose asteroid has the largest `estimated_diameter_avg_km`.
    pub largest_asteroid: Option<NotableApproach>,
}

impl ReportRepository {
    /// Aggregates approach counts and notable approaches for `[start_date, end_date]`.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the underlying queries fail.
    pub async fn get_weekly_summary(
        pool: &PgPool,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<WeeklyReportSummary, sqlx::Error> {
        let (total_approaches, critical_count, high_count, medium_count, low_count): (
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = sqlx::query_as(
            r#"
            SELECT
                COUNT(*),
                COUNT(*) FILTER (WHERE hazard_classification = 'Critical'),
                COUNT(*) FILTER (WHERE hazard_classification = 'High'),
                COUNT(*) FILTER (WHERE hazard_classification = 'Medium'),
                COUNT(*) FILTER (WHERE hazard_classification = 'Low')
            FROM approaches
            WHERE close_approach_date BETWEEN $1 AND $2
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_one(pool)
        .await?;

        let closest_approach = sqlx::query_as::<_, NotableApproach>(
            r#"
            SELECT
                ast.name as asteroid_name,
                a.close_approach_date,
                a.miss_distance_km,
                a.velocity_km_per_h,
                ast.estimated_diameter_avg_km
            FROM approaches a
            JOIN asteroids ast ON a.asteroid_id = ast.id
            WHERE a.close_approach_date BETWEEN $1 AND $2
            ORDER BY a.miss_distance_km ASC
            LIMIT 1
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_optional(pool)
        .await?;

        let fastest_approach = sqlx::query_as::<_, NotableApproach>(
            r#"
            SELECT
                ast.name as asteroid_name,
                a.close_approach_date,
                a.miss_distance_km,
                a.velocity_km_per_h,
                ast.estimated_diameter_avg_km
            FROM approaches a
            JOIN asteroids ast ON a.asteroid_id = ast.id
            WHERE a.close_approach_date BETWEEN $1 AND $2
            ORDER BY a.velocity_km_per_h DESC
            LIMIT 1
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_optional(pool)
        .await?;

        let largest_asteroid = sqlx::query_as::<_, NotableApproach>(
            r#"
            SELECT
                ast.name as asteroid_name,
                a.close_approach_date,
                a.miss_distance_km,
                a.velocity_km_per_h,
                ast.estimated_diameter_avg_km
            FROM approaches a
            JOIN asteroids ast ON a.asteroid_id = ast.id
            WHERE a.close_approach_date BETWEEN $1 AND $2
            ORDER BY ast.estimated_diameter_avg_km DESC
            LIMIT 1
            "#,
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_optional(pool)
        .await?;

        Ok(WeeklyReportSummary {
            start_date,
            end_date,
            total_approaches,
            critical_count,
            high_count,
            medium_count,
            low_count,
            closest_approach,
            fastest_approach,
            largest_asteroid,
        })
    }
}
