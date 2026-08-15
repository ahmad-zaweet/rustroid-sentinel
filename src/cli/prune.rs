//! The `prune` CLI subcommand.
//!
//! Deletes stale `approaches` and `etl_events` rows per the configured
//! [`RetentionConfig`](crate::settings::RetentionConfig), bounding database
//! storage growth on capacity-limited deployments.

use anyhow::Result;
use clap::Args;
use tracing::info;

use crate::database::DatabasePool;
use crate::database::retention::RetentionRepository;
use crate::settings::RustroidSentinelConfig;

/// Arguments for the `prune` CLI subcommand.
#[derive(Args, Debug, Default)]
pub struct PruneArgs {
    /// Report counts of rows that would be deleted without deleting them.
    #[arg(long)]
    pub dry_run: bool,
}

/// Executes the prune command.
///
/// Connects to the database, runs pending migrations, and either reports
/// (`--dry-run`) or deletes rows older than the configured retention
/// thresholds.
///
/// # Errors
///
/// Returns an error if the database connection, migrations, or the
/// count/delete queries fail.
pub async fn execute(args: PruneArgs, settings: RustroidSentinelConfig) -> Result<()> {
    let db_pool = DatabasePool::new(&settings.database).await?;
    db_pool.run_migrations().await?;

    let retention = &settings.etl.retention;

    if args.dry_run {
        let result = RetentionRepository::count_prunable(db_pool.pool(), retention).await?;
        info!(
            approaches = result.approaches,
            etl_events = result.etl_events,
            "DRY RUN: rows eligible for pruning"
        );
        println!(
            "Would prune {} approaches and {} etl_events",
            result.approaches, result.etl_events
        );
        return Ok(());
    }

    let result = RetentionRepository::prune(db_pool.pool(), retention).await?;
    info!(
        approaches = result.approaches,
        etl_events = result.etl_events,
        "Pruned stale rows"
    );
    println!(
        "Pruned {} approaches and {} etl_events",
        result.approaches, result.etl_events
    );

    Ok(())
}
