//! The `orbits` CLI subcommand.
//!
//! Fetches orbital elements (eccentricity, semi-major axis, orbit class,
//! spectral class, albedo) for asteroids from JPL's Small-Body Database API
//! and stores them in `asteroid_orbits`. Unlike `sentry`, every asteroid is a
//! candidate — orbital elements are catalog metadata, not tied to
//! impact-risk status — so each still gets `orbit_checked_at` stamped so it
//! isn't re-queried until it goes stale.

use anyhow::Result;
use chrono::Utc;
use clap::Args;
use futures_util::{Stream, StreamExt};
use sqlx::PgPool;
use std::pin::Pin;
use tokio::task::JoinSet;
use tracing::{info, warn};
use uuid::Uuid;

use crate::api::client::SharedHttpClient;
use crate::database::DatabasePool;
use crate::database::repository::AsteroidRepository;
use crate::nasa::error::NasaApiError;
use crate::nasa::jpl_sbdb::api::JplSbdbApi;
use crate::nasa::jpl_sbdb::responses::SbdbOrbitSummary;
use crate::settings::RustroidSentinelConfig;

/// Number of orbit updates buffered before flushing to the DB, so a large
/// candidate set never sits fully in memory as pending updates.
const UPDATE_FLUSH_SIZE: usize = 200;

/// Concurrent SBDB lookups in flight at once. SBDB exposes no rate-limit
/// headers, so this — plus the per-dispatch delay — is a self-imposed cap
/// rather than one derived from API feedback. Lower than `sentry`'s
/// concurrency: SBDB throttles (503) under load that Sentry tolerates, and
/// sustained hammering past that throttle escalates to a hard 403 block.
const LOOKUP_CONCURRENCY: usize = 2;

/// Consecutive lookup failures (after which a request stops even being
/// attempted) that trigger an early abort. A run that's being throttled or
/// blocked should stop rather than plow through the rest of the candidate
/// list making the block worse — errors here are consecutive, not total, so
/// occasional one-off failures scattered across a run don't trip it.
const CONSECUTIVE_ERROR_ABORT: u32 = 5;

/// Arguments for the `orbits` CLI subcommand.
#[derive(Args, Debug, Default)]
pub struct OrbitsArgs {
    /// Re-fetch orbital elements for every asteroid, ignoring `orbit_checked_at`.
    #[arg(long)]
    pub recompute: bool,
}

/// One completed (or panicked) SBDB lookup task's outcome.
type LookupOutcome = (Uuid, String, Result<Option<SbdbOrbitSummary>, NasaApiError>);

/// A DB-backed stream of orbit-check candidates, boxed so `run_orbit_checks`
/// can accept either the full-catalog staleness sweep or the small
/// pipeline-scoped id list behind one signature.
type OrbitCandidateStream<'a> =
    Pin<Box<dyn Stream<Item = Result<(Uuid, String), sqlx::Error>> + Send + 'a>>;

/// Spawns a single SBDB lookup as its own task, so a panic inside the HTTP
/// client or JSON parsing is isolated (caught as a `JoinError` by the
/// caller) rather than unwinding the whole `orbits` run.
fn spawn_lookup(
    in_flight: &mut JoinSet<LookupOutcome>,
    api: JplSbdbApi,
    id: Uuid,
    neo_reference_id: String,
) {
    in_flight.spawn(async move {
        let result = api.lookup_by_spk(&neo_reference_id).await;
        (id, neo_reference_id, result)
    });
}

/// Executes the orbits command.
///
/// # Errors
///
/// Returns an error if the database connection, migrations, or the
/// candidate-selection/update queries fail. Individual SBDB lookup failures
/// (including a lookup task panicking) are logged and skipped rather than
/// aborting the whole run.
pub async fn execute(args: OrbitsArgs, settings: RustroidSentinelConfig) -> Result<()> {
    let db_pool = DatabasePool::new(&settings.database).await?;
    db_pool.run_migrations().await?;

    let stale_before = if args.recompute {
        Utc::now()
    } else {
        Utc::now() - chrono::Duration::days(settings.jpl_sbdb.stale_days as i64)
    };

    let total_candidates =
        AsteroidRepository::count_asteroids_needing_orbit_check(db_pool.pool(), stale_before)
            .await?;
    info!(total_candidates, "Fetching orbital elements from JPL SBDB");

    if total_candidates == 0 {
        println!("No asteroids due for an orbit check.");
        return Ok(());
    }

    // Streamed straight from Postgres rather than collected into a `Vec` up
    // front — every asteroid in the catalog is a candidate here (unlike
    // `sentry`'s hazard-flagged subset), so memory use would otherwise scale
    // with the whole catalog for no reason.
    let candidates = Box::pin(AsteroidRepository::stream_asteroids_needing_orbit_check(
        db_pool.pool(),
        stale_before,
    ));

    let summary = run_orbit_checks(db_pool.pool(), &settings, candidates).await?;

    info!(
        checked = summary.checked,
        total_candidates,
        matched = summary.matched,
        errored = summary.errored,
        updated = summary.updated,
        aborted = summary.aborted,
        "Orbit check complete"
    );
    println!(
        "Checked {} of {} candidate asteroids: {} orbit records found, {} lookups failed, {} rows updated{}",
        summary.checked,
        total_candidates,
        summary.matched,
        summary.errored,
        summary.updated,
        if summary.aborted {
            " (stopped early — SBDB looked rate-limited or blocked)"
        } else {
            ""
        }
    );

    if summary.aborted {
        anyhow::bail!(
            "Aborted after {CONSECUTIVE_ERROR_ABORT} consecutive SBDB failures; re-run later, or with a \
             longer jpl_sbdb.request_delay_ms"
        );
    }

    Ok(())
}

/// Scopes an orbit-elements check to exactly the given asteroid ids,
/// skipping the staleness-gated candidate sweep entirely. Intended for the
/// `pipeline` command, which calls this right after `load` with only the
/// ids of asteroids newly inserted in that run — a small, cheap set
/// compared to a full catalog sweep.
///
/// Never fails the caller's pipeline: DB/query errors, individual lookup
/// failures, and even the consecutive-failure circuit breaker are all
/// logged and this returns `Ok(())` regardless, so an SBDB outage never
/// aborts the broader ETL run.
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

    let candidates = AsteroidRepository::orbit_candidates_for_ids(db_pool.pool(), &ids).await?;
    if candidates.is_empty() {
        return Ok(());
    }

    let stream = Box::pin(futures_util::stream::iter(candidates.into_iter().map(Ok)));
    let summary = run_orbit_checks(db_pool.pool(), &settings, stream).await?;

    info!(
        checked = summary.checked,
        matched = summary.matched,
        errored = summary.errored,
        updated = summary.updated,
        aborted = summary.aborted,
        "Pipeline-scoped orbit enrichment complete"
    );

    Ok(())
}

/// Aggregate outcome of a batch of SBDB orbit lookups.
struct OrbitCheckSummary {
    checked: u32,
    matched: u32,
    errored: u32,
    updated: u64,
    aborted: bool,
}

/// Per-outcome counters accumulated while draining lookups, grouped so
/// `run_orbit_checks` doesn't carry four separate `&mut` locals through its
/// loop body.
#[derive(Default)]
struct RunTally {
    checked: u32,
    matched: u32,
    errored: u32,
    consecutive_errors: u32,
}

/// Applies one joined SBDB lookup result to `tally`/`pending`: increments
/// the right counters, logs failures, and queues successful (or
/// not-found) lookups for the next DB flush. Split out of
/// `run_orbit_checks` so its join-error match nested inside a
/// lookup-result match doesn't count against that function's cognitive
/// complexity.
fn apply_outcome(
    joined: Result<LookupOutcome, tokio::task::JoinError>,
    tally: &mut RunTally,
    pending: &mut Vec<(Uuid, SbdbOrbitSummary)>,
) {
    let (id, neo_reference_id, result) = match joined {
        Ok(outcome) => outcome,
        Err(join_err) => {
            tally.errored += 1;
            tally.consecutive_errors += 1;
            warn!(error = %join_err, "SBDB lookup task panicked, skipping");
            return;
        }
    };

    tally.checked += 1;
    match result {
        Ok(Some(summary)) => {
            tally.matched += 1;
            tally.consecutive_errors = 0;
            info!(
                neo_reference_id = %neo_reference_id,
                orbit_class = ?summary.orbit_class,
                "Orbit record found, queuing update"
            );
            pending.push((id, summary));
        }
        Ok(None) => {
            tally.consecutive_errors = 0;
            pending.push((id, Default::default()));
        }
        Err(e) => {
            tally.errored += 1;
            tally.consecutive_errors += 1;
            warn!(neo_reference_id = %neo_reference_id, error = %e, "SBDB lookup failed, skipping");
        }
    }
}

/// Pulls the next candidate off `candidates` and spawns its lookup, if any
/// remain. Shared by the priming loop and the steady-state refill in
/// `run_orbit_checks` so both handle the stream's DB-error case identically.
async fn spawn_next(
    candidates: &mut OrbitCandidateStream<'_>,
    sbdb_api: &JplSbdbApi,
    in_flight: &mut JoinSet<LookupOutcome>,
) -> Result<()> {
    match candidates.next().await {
        Some(Ok((id, neo_reference_id))) => {
            spawn_lookup(in_flight, sbdb_api.clone(), id, neo_reference_id);
        }
        Some(Err(db_err)) => return Err(db_err.into()),
        None => {}
    }
    Ok(())
}

/// Drains `candidates` (keeping [`LOOKUP_CONCURRENCY`] SBDB lookups in
/// flight at all times) and writes results back to `asteroid_orbits`,
/// flushing every [`UPDATE_FLUSH_SIZE`] pending updates. Individual lookup
/// failures are logged and skipped; [`CONSECUTIVE_ERROR_ABORT`] consecutive
/// failures stop the drain early (reported via `aborted` on the returned
/// summary) rather than continuing to hammer a throttled/blocked API.
async fn run_orbit_checks(
    pool: &PgPool,
    settings: &RustroidSentinelConfig,
    mut candidates: OrbitCandidateStream<'_>,
) -> Result<OrbitCheckSummary> {
    let http_client = SharedHttpClient::new(settings).await?;
    let sbdb_api = JplSbdbApi::new(http_client, settings.jpl_sbdb.clone());
    let request_delay = std::time::Duration::from_millis(settings.jpl_sbdb.request_delay_ms);

    let mut pending = Vec::with_capacity(UPDATE_FLUSH_SIZE);
    let mut tally = RunTally::default();
    let mut updated = 0u64;
    let mut aborted = false;

    let mut in_flight = JoinSet::new();

    // Prime the pool: keep exactly `LOOKUP_CONCURRENCY` lookups in flight at
    // all times by refilling one task per completion below, instead of
    // awaiting a fixed-size chunk and sleeping before starting the next one.
    // That chunk-then-sleep shape let one slow lookup in a pair stall a
    // second, already-finished slot until the whole pair completed plus the
    // delay; refilling immediately keeps both slots continuously busy.
    for _ in 0..LOOKUP_CONCURRENCY {
        spawn_next(&mut candidates, &sbdb_api, &mut in_flight).await?;
    }

    while let Some(joined) = in_flight.join_next().await {
        apply_outcome(joined, &mut tally, &mut pending);

        if pending.len() >= UPDATE_FLUSH_SIZE {
            updated += AsteroidRepository::upsert_asteroid_orbits(pool, &pending).await?;
            pending.clear();
        }

        if tally.consecutive_errors >= CONSECUTIVE_ERROR_ABORT {
            warn!(
                consecutive_errors = tally.consecutive_errors,
                "Too many consecutive SBDB failures in a row (likely rate-limited or blocked) — \
                 stopping early rather than hammering the API further"
            );
            aborted = true;
            break;
        }

        // Paces new dispatches rather than the whole batch, so the other
        // in-flight lookup isn't held up waiting on this sleep.
        tokio::time::sleep(request_delay).await;

        spawn_next(&mut candidates, &sbdb_api, &mut in_flight).await?;
    }

    updated += AsteroidRepository::upsert_asteroid_orbits(pool, &pending).await?;

    Ok(OrbitCheckSummary {
        checked: tally.checked,
        matched: tally.matched,
        errored: tally.errored,
        updated,
        aborted,
    })
}
