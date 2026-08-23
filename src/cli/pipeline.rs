//! The `pipeline` CLI subcommand.
//!
//! Runs the full scheduled job in one process: `extract -> transform -> load
//! -> alert -> sentry-enrich -> orbit-enrich -> prune -> vectorize -> report
//! (Sundays only)`. On a database metered by compute-hour, waking it once for
//! all nine stages costs roughly a ninth of waking it once per stage, so this
//! is the entry point the scheduler should call instead of invoking the
//! individual subcommands separately.
//!
//! The two enrichment stages are scoped to only the asteroids newly inserted
//! by this run's `load` stage (not a full-catalog staleness sweep — that
//! stays a separately-scheduled job) and are non-fatal: a JPL Sentry/SBDB
//! outage is logged and skipped rather than aborting the pipeline, since
//! neither is essential to the ETL run's core data being correct.

use anyhow::{Context, Result};
use chrono::{Datelike, Duration, NaiveDate, Utc, Weekday};
use clap::Args;
use std::path::PathBuf;
use tracing::{info, warn};
use uuid::Uuid;

use crate::cli::alert;
use crate::cli::extract::{self, ExtractArgs};
use crate::cli::load::{self, LoadArgs};
use crate::cli::orbits;
use crate::cli::prune::{self, PruneArgs};
use crate::cli::report::{self, ReportArgs};
use crate::cli::sentry;
use crate::cli::transform::{self, TransformArgs};
use crate::cli::vectorize::{self, VectorizeArgs};
use crate::settings::RustroidSentinelConfig;

const RAW_DIR: &str = "data/raw";
const TRANSFORMED_DIR: &str = "data/transformed";

/// Arguments for the `pipeline` CLI subcommand.
#[derive(Args, Debug, Default)]
pub struct PipelineArgs {
    /// Send the weekly report stage even if today isn't Sunday.
    #[arg(long)]
    pub force_report: bool,
    /// Skip the weekly report stage entirely.
    #[arg(long)]
    pub skip_report: bool,
}

/// Executes the full pipeline: extract, transform, load, alert,
/// sentry-enrich, orbit-enrich, prune, vectorize, and (Sundays only) the
/// weekly report.
///
/// Each stage runs to completion before the next starts. Extract, transform,
/// load, alert, prune, vectorize, and report failures abort the pipeline
/// rather than letting later stages run on incomplete data. The two
/// enrichment stages are the exception: they're logged and skipped on
/// failure so a JPL Sentry/SBDB outage never aborts the broader run.
///
/// # Errors
///
/// Returns an error if extract, transform, load, alert, prune, vectorize, or
/// report fails.
pub async fn execute(args: PipelineArgs, settings: RustroidSentinelConfig) -> Result<()> {
    let today = Utc::now().date_naive();

    stage_extract(&settings, today).await?;
    stage_transform().await?;
    let new_asteroid_ids = stage_load(&settings).await?;
    stage_alert(&settings).await?;
    stage_enrichment(&settings, new_asteroid_ids).await;
    stage_prune_vectorize(&settings).await?;
    stage_report(&args, settings, today).await?;

    info!("Pipeline completed successfully");
    Ok(())
}

/// Stage 1/9. Fetches the configured lookback/lookahead window from NASA.
async fn stage_extract(settings: &RustroidSentinelConfig, today: NaiveDate) -> Result<()> {
    info!("Pipeline stage 1/9: extract");
    let extract_args = ExtractArgs {
        start_date: today - Duration::days(settings.etl.lookback_days as i64),
        end_date: today + Duration::days(settings.etl.lookahead_days as i64),
        output: Some(PathBuf::from(RAW_DIR)),
        batch_size: Some(7),
        force: false,
        dry_run: false,
    };
    extract::execute(extract_args, settings.clone())
        .await
        .context("pipeline: extract stage failed")
}

/// Stage 2/9. Converts the raw JSON just extracted into domain-model NDJSON.
async fn stage_transform() -> Result<()> {
    info!("Pipeline stage 2/9: transform");
    let transform_args = TransformArgs {
        input: PathBuf::from(RAW_DIR),
        output: PathBuf::from(TRANSFORMED_DIR),
        force: false,
        dry_run: false,
    };
    transform::execute(transform_args)
        .await
        .context("pipeline: transform stage failed")
}

/// Stage 3/9. Loads the transformed NDJSON into Postgres, returning the ids
/// of asteroids newly inserted by this run (the enrichment stages' scope).
async fn stage_load(settings: &RustroidSentinelConfig) -> Result<Vec<Uuid>> {
    info!("Pipeline stage 3/9: load");
    let load_args = LoadArgs {
        input: PathBuf::from(TRANSFORMED_DIR),
        force: false,
        dry_run: false,
    };
    load::execute(load_args, settings.clone())
        .await
        .context("pipeline: load stage failed")
}

/// Stage 4/9. Scans for un-alerted hazardous approaches and sends Discord
/// pings. Idempotent regardless of run frequency: it only alerts on
/// approaches without a matching row in the `alerts` table.
async fn stage_alert(settings: &RustroidSentinelConfig) -> Result<()> {
    info!("Pipeline stage 4/9: alert");
    alert::execute(alert::AlertArgs {}, settings.clone())
        .await
        .context("pipeline: alert stage failed")
}

/// Stages 5-6/9. JPL Sentry and SBDB enrichment for the newly-loaded
/// asteroids. Both are non-fatal: an outage is logged and skipped rather
/// than aborting the pipeline.
async fn stage_enrichment(settings: &RustroidSentinelConfig, new_asteroid_ids: Vec<Uuid>) {
    stage_sentry_enrichment(settings, new_asteroid_ids.clone()).await;
    stage_orbit_enrichment(settings, new_asteroid_ids).await;
}

/// Stage 5/9. JPL Sentry enrichment; logged and skipped on failure.
async fn stage_sentry_enrichment(settings: &RustroidSentinelConfig, new_asteroid_ids: Vec<Uuid>) {
    info!(
        count = new_asteroid_ids.len(),
        "Pipeline stage 5/9: sentry-scale enrichment"
    );
    if let Err(e) = sentry::execute_for_ids(settings.clone(), new_asteroid_ids).await {
        warn!(error = %e, "pipeline: sentry enrichment stage failed, continuing");
    }
}

/// Stage 6/9. JPL SBDB orbit enrichment; logged and skipped on failure.
async fn stage_orbit_enrichment(settings: &RustroidSentinelConfig, new_asteroid_ids: Vec<Uuid>) {
    info!(
        count = new_asteroid_ids.len(),
        "Pipeline stage 6/9: orbit-elements enrichment"
    );
    if let Err(e) = orbits::execute_for_ids(settings.clone(), new_asteroid_ids).await {
        warn!(error = %e, "pipeline: orbit enrichment stage failed, continuing");
    }
}

/// Stages 7-8/9. Retention pruning followed by a full embedding recompute.
async fn stage_prune_vectorize(settings: &RustroidSentinelConfig) -> Result<()> {
    info!("Pipeline stage 7/9: prune");
    prune::execute(PruneArgs::default(), settings.clone())
        .await
        .context("pipeline: prune stage failed")?;

    info!("Pipeline stage 8/9: vectorize");
    vectorize::execute(VectorizeArgs::default(), settings.clone())
        .await
        .context("pipeline: vectorize stage failed")
}

/// Stage 9/9. Sends the weekly Discord report, unless skipped or it isn't
/// Sunday (and `--force-report` wasn't passed).
async fn stage_report(
    args: &PipelineArgs,
    settings: RustroidSentinelConfig,
    today: NaiveDate,
) -> Result<()> {
    if args.skip_report {
        info!("Pipeline stage 9/9: report skipped (--skip-report)");
        return Ok(());
    }

    if args.force_report || today.weekday() == Weekday::Sun {
        info!("Pipeline stage 9/9: report");
        report::execute(ReportArgs::default(), settings)
            .await
            .context("pipeline: report stage failed")
    } else {
        info!("Pipeline stage 9/9: report skipped (not Sunday)");
        Ok(())
    }
}
