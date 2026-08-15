//! Retention/pruning queries used by the `prune` CLI command.
//!
//! Bounds storage growth on capacity-limited databases (e.g. Neon free
//! tier's 0.5 GB limit) by deleting old `approaches` rows and trimming the
//! `etl_events` audit log.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;

use crate::settings::RetentionConfig;

/// Repository for data-retention operations.
pub struct RetentionRepository;

/// Counts of rows eligible for pruning, or actually deleted.
#[derive(Debug, Default, Clone, Copy)]
pub struct PruneResult {
    /// Number of `approaches` rows affected.
    pub approaches: u64,
    /// Number of `etl_events` rows affected.
    pub etl_events: u64,
}

impl RetentionRepository {
    fn approach_cutoff(config: &RetentionConfig) -> NaiveDate {
        Utc::now().date_naive()
            - chrono::Duration::days(i64::from(config.approach_retention_years) * 365)
    }

    fn etl_event_cutoff(config: &RetentionConfig) -> DateTime<Utc> {
        Utc::now() - chrono::Duration::days(i64::from(config.etl_event_retention_days))
    }

    /// Counts rows eligible for pruning without deleting anything.
    ///
    /// # Errors
    ///
    /// Returns a [`sqlx::Error`] if either count query fails.
    pub async fn count_prunable(
        pool: &PgPool,
        config: &RetentionConfig,
    ) -> Result<PruneResult, sqlx::Error> {
        let approach_cutoff = Self::approach_cutoff(config);
        let etl_event_cutoff = Self::etl_event_cutoff(config);

        let approaches: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM approaches WHERE close_approach_date < $1")
                .bind(approach_cutoff)
                .fetch_one(pool)
                .await?;

        let etl_events: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM etl_events
            WHERE started_at < $1
              AND id NOT IN (
                  SELECT id FROM etl_events ORDER BY started_at DESC LIMIT $2
              )
            "#,
        )
        .bind(etl_event_cutoff)
        .bind(i64::from(config.etl_events_keep_min))
        .fetch_one(pool)
        .await?;

        Ok(PruneResult {
            approaches: u64::try_from(approaches.0).unwrap_or(0),
            etl_events: u64::try_from(etl_events.0).unwrap_or(0),
        })
    }

    /// Deletes prunable rows and returns the number of rows removed.
    ///
    /// `approaches` older than `retention.approach_retention_years` are
    /// deleted outright (cascades to any `alerts` referencing them).
    /// `etl_events` older than `retention.etl_event_retention_days` are
    /// deleted, except the most recent `retention.etl_events_keep_min` rows,
    /// which are always kept regardless of age.
    ///
    /// # Errors
    ///
    /// Returns a [`sqlx::Error`] if either delete query fails.
    pub async fn prune(
        pool: &PgPool,
        config: &RetentionConfig,
    ) -> Result<PruneResult, sqlx::Error> {
        let approach_cutoff = Self::approach_cutoff(config);
        let etl_event_cutoff = Self::etl_event_cutoff(config);

        let approaches = sqlx::query("DELETE FROM approaches WHERE close_approach_date < $1")
            .bind(approach_cutoff)
            .execute(pool)
            .await?
            .rows_affected();

        let etl_events = sqlx::query(
            r#"
            DELETE FROM etl_events
            WHERE started_at < $1
              AND id NOT IN (
                  SELECT id FROM etl_events ORDER BY started_at DESC LIMIT $2
              )
            "#,
        )
        .bind(etl_event_cutoff)
        .bind(i64::from(config.etl_events_keep_min))
        .execute(pool)
        .await?
        .rows_affected();

        Ok(PruneResult {
            approaches,
            etl_events,
        })
    }
}
