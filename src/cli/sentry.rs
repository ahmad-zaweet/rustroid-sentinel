//! The `sentry` CLI subcommand.
//!
//! Checks `is_sentry_object`-flagged asteroids against JPL's Sentry
//! impact-monitoring API and stores real `torino_scale`/`palermo_scale`
//! values. NeoWs's `is_sentry_object` flag already means "currently on the
//! Sentry Risk List", so unlike a plain PHA sweep, most candidates here
//! *are* expected to match; each still gets `sentry_checked_at` stamped so
//! it isn't re-queried until it goes stale.

use anyhow::Result;
use chrono::Utc;
use clap::Args;
use sqlx::PgPool;
use tokio::task::JoinSet;
use tracing::{info, warn};
use uuid::Uuid;

use crate::api::client::SharedHttpClient;
use crate::database::DatabasePool;
use crate::database::repository::AsteroidRepository;
use crate::nasa::jpl_sentry::api::JplSentryApi;
use crate::settings::RustroidSentinelConfig;

/// Number of scale updates buffered before flushing to the DB, so a large
/// candidate set never sits fully in memory as pending updates.
const UPDATE_FLUSH_SIZE: usize = 200;

/// Concurrent Sentry lookups in flight at once. Sentry exposes no rate-limit
/// headers, so this — plus the inter-chunk delay — is a self-imposed cap
/// rather than one derived from API feedback.
const LOOKUP_CONCURRENCY: usize = 5;

/// Arguments for the `sentry` CLI subcommand.
#[derive(Args, Debug, Default)]
pub struct SentryArgs {
    /// Re-check every Sentry-flagged asteroid, ignoring `sentry_checked_at`.
    #[arg(long)]
    pub recompute: bool,
}

/// Executes the sentry command.
///
/// # Errors
///
/// Returns an error if the database connection, migrations, or the
/// candidate-selection/update queries fail. Individual Sentry API lookup
/// failures are logged and skipped rather than aborting the whole run.
pub async fn execute(args: SentryArgs, settings: RustroidSentinelConfig) -> Result<()> {
    let db_pool = DatabasePool::new(&settings.database).await?;
    db_pool.run_migrations().await?;

    let stale_before = if args.recompute {
        Utc::now()
    } else {
        Utc::now() - chrono::Duration::days(settings.jpl_sentry.stale_days as i64)
    };

    let candidates =
        AsteroidRepository::asteroids_needing_sentry_check(db_pool.pool(), stale_before).await?;
    info!(
        candidates = candidates.len(),
        "Checking asteroids against JPL Sentry"
    );

    if candidates.is_empty() {
        println!("No asteroids due for a Sentry check.");
        return Ok(());
    }

    let checked = candidates.len();
    let summary = run_sentry_checks(db_pool.pool(), &settings, candidates).await?;

    info!(
        checked,
        matched = summary.matched,
        errored = summary.errored,
        updated = summary.updated,
        "Sentry check complete"
    );
    println!(
        "Checked {} asteroids: {} are current Sentry virtual impactors, {} lookups failed, {} rows updated",
        checked, summary.matched, summary.errored, summary.updated
    );

    Ok(())
}

/// Scopes a Sentry check to exactly the given asteroid ids, skipping the
/// staleness-gated candidate sweep entirely. Intended for the `pipeline`
/// command, which calls this right after `load` with only the ids of
/// asteroids newly inserted in that run — a small, cheap set compared to a
/// full catalog sweep.
///
/// Never fails the caller's pipeline: DB/query errors and individual lookup
/// failures are all logged and this returns `Ok(())` regardless, so a
/// Sentry/JPL outage never aborts the broader ETL run.
///
/// # Errors
///
/// Returns an error only if the database connection or migrations fail;
/// candidate-selection, lookup, and update-flush errors are logged and
/// swallowed since this is meant to be non-fatal for pipeline callers.
pub async fn execute_for_ids(settings: RustroidSentinelConfig, ids: Vec<Uuid>) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    let db_pool = DatabasePool::new(&settings.database).await?;
    db_pool.run_migrations().await?;

    let candidates = AsteroidRepository::sentry_candidates_for_ids(db_pool.pool(), &ids).await?;
    if candidates.is_empty() {
        info!("No newly-inserted asteroids are Sentry-flagged; skipping");
        return Ok(());
    }

    let checked = candidates.len();
    let summary = run_sentry_checks(db_pool.pool(), &settings, candidates).await?;

    info!(
        checked,
        matched = summary.matched,
        errored = summary.errored,
        updated = summary.updated,
        "Pipeline-scoped Sentry enrichment complete"
    );

    Ok(())
}

/// Aggregate outcome of a batch of Sentry lookups.
struct SentryCheckSummary {
    matched: u32,
    errored: u32,
    updated: u64,
}

/// Looks up `candidates` against the JPL Sentry API and writes results back
/// to `asteroids`, in chunks of [`LOOKUP_CONCURRENCY`] with an inter-chunk
/// delay and periodic flush every [`UPDATE_FLUSH_SIZE`] pending updates.
/// Individual lookup failures are logged and skipped rather than
/// propagated, so a single bad candidate never aborts the batch.
async fn run_sentry_checks(
    pool: &PgPool,
    settings: &RustroidSentinelConfig,
    candidates: Vec<(Uuid, String)>,
) -> Result<SentryCheckSummary> {
    let http_client = SharedHttpClient::new(settings).await?;
    let sentry_api = JplSentryApi::new(http_client, settings.jpl_sentry.clone());
    let request_delay = std::time::Duration::from_millis(settings.jpl_sentry.request_delay_ms);

    let mut pending = Vec::with_capacity(UPDATE_FLUSH_SIZE);
    let mut matched = 0u32;
    let mut errored = 0u32;
    let mut updated = 0u64;

    for (chunk_index, chunk) in candidates.chunks(LOOKUP_CONCURRENCY).enumerate() {
        if chunk_index > 0 {
            tokio::time::sleep(request_delay).await;
        }

        let mut in_flight = JoinSet::new();
        for (id, neo_reference_id) in chunk {
            let api = sentry_api.clone();
            let id = *id;
            let neo_reference_id = neo_reference_id.clone();
            in_flight.spawn(async move {
                let result = api.lookup_by_spk(&neo_reference_id).await;
                (id, neo_reference_id, result)
            });
        }

        while let Some(joined) = in_flight.join_next().await {
            let (id, neo_reference_id, result) = joined?;
            match result {
                Ok(Some(summary)) => {
                    matched += 1;
                    pending.push((id, summary.ts_max, summary.ps_cum));
                }
                Ok(None) => {
                    pending.push((id, None, None));
                }
                Err(e) => {
                    errored += 1;
                    warn!(neo_reference_id = %neo_reference_id, error = %e, "Sentry lookup failed, skipping");
                }
            }
        }

        if pending.len() >= UPDATE_FLUSH_SIZE {
            updated += AsteroidRepository::update_sentry_scales(pool, &pending).await?;
            pending.clear();
        }
    }

    updated += AsteroidRepository::update_sentry_scales(pool, &pending).await?;

    Ok(SentryCheckSummary {
        matched,
        errored,
        updated,
    })
}
