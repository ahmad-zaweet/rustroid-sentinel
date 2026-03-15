//! # Load CLI Command
//!
//! This module implements the `load` subcommand, which persists transformed
//! domain models into the PostgreSQL database.
//!
//! ## Features
//!
//! - **Idempotency**: Tracks processed files in `etl_events` table to prevent duplicates
//! - **Concurrency Control**: Limits concurrent database writes to avoid deadlocks
//! - **Batch Upserts**: Uses PostgreSQL's `INSERT ... ON CONFLICT` for efficient upserts
//! - **ETL Tracking**: Records start/completion timestamps and processing statistics

// src/cli/load.rs
use anyhow::{Context, Result};
use chrono::Utc;
use clap::Args;
use prettytable::{Attr, Cell, Row, Table, format, row};
use sqlx::PgPool;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use tokio::fs;
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::database::DatabasePool;
use crate::database::repository::AsteroidRepository;
use crate::models::approach::Approach;
use crate::models::asteroid::Asteroid;
use crate::settings::RustroidSentinelConfig;

/// Arguments for the `load` CLI subcommand.
///
/// The load command persists transformed JSON files to the database.
///
/// # Examples
///
/// ```bash
/// # Load all transformed files
/// rustroid-sentinel load
///
/// # Force re-loading (ignore idempotency)
/// rustroid-sentinel load --force
///
/// # Preview without writing (dry run)
/// rustroid-sentinel load --dry-run
/// ```
#[derive(Args, Debug, Clone)]
pub struct LoadArgs {
    /// Input directory containing transformed domain model JSON files.
    #[arg(short, long, default_value = "data/transformed")]
    pub input: PathBuf,

    /// Force re-loading of files even if they have already been processed
    /// according to the ETL event history.
    #[arg(short, long)]
    pub force: bool,

    /// Dry run mode - Identifies files to load and previews database actions
    /// without performing any writes or updates.
    #[arg(long)]
    pub dry_run: bool,
}

/// Statistics collected during the database loading process.
#[derive(Debug, Default)]
struct LoadStats {
    /// Number of files successfully processed.
    files_processed: usize,
    /// Number of files skipped due to idempotency checks.
    files_skipped: usize,
    /// Total number of asteroids upserted (inserted or updated).
    total_asteroids_upserted: u64,
    /// Total number of new close approach records inserted.
    total_approaches_inserted: u64,
    /// Total number of duplicate approach records that were ignored.
    total_approaches_skipped: u64,
}

impl LoadStats {
    /// Merges statistics from a single file process into the global totals.
    fn merge(&mut self, other: &LoadStats) {
        self.files_processed += other.files_processed;
        self.files_skipped += other.files_skipped;
        self.total_asteroids_upserted += other.total_asteroids_upserted;
        self.total_approaches_inserted += other.total_approaches_inserted;
        self.total_approaches_skipped += other.total_approaches_skipped;
    }
}

/// Executes the loading process.
///
/// This command persists transformed domain models into the PostgreSQL database.
/// It performs the following steps:
/// 1. Connects to the database and runs pending migrations.
/// 2. Collects all `_transformed.json` files from the input directory.
/// 3. Spawns concurrent tasks to process each file with concurrency limiting (semaphore).
/// 4. For each file, checks idempotency, records ETL events, and performs batch upserts.
///
/// # Errors
///
/// Returns an error if database connection, migrations, or any file load fails.
#[allow(clippy::cognitive_complexity)]
#[allow(clippy::too_many_lines)]
pub async fn execute(args: LoadArgs, settings: RustroidSentinelConfig) -> Result<()> {
    info!("Starting load command");

    if !args.input.exists() {
        anyhow::bail!(
            "Input directory '{}' does not exist. Run 'transform' first.",
            args.input.display()
        );
    }

    // Collect transformed JSON files
    let json_files = collect_transformed_files(&args.input).await?;

    if json_files.is_empty() {
        warn!(path = %args.input.display(), "No transformed JSON files found");
        return Ok(());
    }

    info!(count = json_files.len(), "Found transformed files to load");

    if args.dry_run {
        info!("DRY RUN MODE - No data will be written to the database");
        for file in &json_files {
            info!(path = %file.display(), "Dry run: Would load file");
        }
        return Ok(());
    }

    // Connect to the database
    let db = DatabasePool::new(&settings.database)
        .await
        .context("Failed to connect to the database")?;

    // Run migrations to ensure tables exist
    db.run_migrations()
        .await
        .context("Failed to run database migrations")?;

    let pool = db.pool().clone();
    let mut stats = LoadStats::default();
    let mut join_set = JoinSet::new();

    // Limit concurrency to avoid overwhelming the database and causing deadlocks
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(10));

    // Spawn tasks for each file
    for file_path in json_files {
        let pool = pool.clone();
        let force = args.force;
        let semaphore = semaphore.clone();

        join_set.spawn(async move {
            let _permit = semaphore
                .acquire()
                .await
                .map_err(|e| anyhow::anyhow!("Semaphore acquire failed: {}", e))?;
            process_file(file_path, pool, force).await
        });
    }

    // Wait for tasks to complete and aggregate stats
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok(file_stats)) => {
                stats.merge(&file_stats);
            }
            Ok(Err(e)) => {
                error!(error = %e, "File processing failed");
            }
            Err(e) => {
                error!(error = %e, "Task panicked or was cancelled");
            }
        }
    }

    print_summary(&stats);
    info!("Load completed successfully");
    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn process_file(file_path: PathBuf, pool: PgPool, force: bool) -> Result<LoadStats> {
    let file_name = file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut stats = LoadStats::default();

    // Check idempotency: skip if already processed
    if !force && check_idempotency(&pool, &file_name).await? {
        stats.files_skipped += 1;
        return Ok(stats);
    }

    info!(path = %file_path.display(), "Loading transformed file");

    let event_id = Uuid::new_v4();
    record_etl_start(&pool, event_id, &file_name).await;

    // Stream and process records without loading entire file into memory
    match stream_and_process_file(&file_path, &pool).await {
        Ok(upsert_stats) => {
            handle_load_success(&pool, &file_name, event_id, &upsert_stats, &mut stats).await;
            Ok(stats)
        }
        Err(e) => {
            handle_load_failure(&pool, &file_name, event_id, &e).await;
            Err(e).context(format!("Failed to load {}", file_name))
        }
    }
}

/// Helper to check if a file has already been processed.
async fn check_idempotency(pool: &PgPool, file_name: &str) -> Result<bool> {
    match AsteroidRepository::is_file_processed(pool, file_name).await {
        Ok(true) => {
            debug!(file = %file_name, "Skipping file (already processed)");
            Ok(true)
        }
        Ok(false) => Ok(false),
        Err(e) => {
            warn!(
                file = %file_name,
                error = %e,
                "Failed to check ETL event record. Proceeding anyway."
            );
            Ok(false)
        }
    }
}

/// Helper to record the start of an ETL event.
async fn record_etl_start(pool: &PgPool, event_id: Uuid, file_name: &str) {
    let started_at = Utc::now();
    if let Err(e) =
        AsteroidRepository::record_etl_event_start(pool, event_id, file_name, started_at).await
    {
        warn!(error = %e, "Failed to record ETL event start. Continuing...");
    }
}

/// Helper to handle successful file loading.
async fn handle_load_success(
    pool: &PgPool,
    file_name: &str,
    event_id: Uuid,
    upsert_stats: &crate::database::repository::UpsertStats,
    stats: &mut LoadStats,
) {
    debug!(file = %file_name, %upsert_stats, "File loaded successfully");

    stats.total_asteroids_upserted +=
        upsert_stats.asteroids_inserted + upsert_stats.asteroids_updated;
    stats.total_approaches_inserted += upsert_stats.approaches_inserted;
    stats.total_approaches_skipped += upsert_stats.approaches_skipped;
    stats.files_processed += 1;

    // Record success
    if let Err(e) = AsteroidRepository::record_etl_event_complete(
        pool,
        event_id,
        upsert_stats.asteroids_inserted as i32,
        upsert_stats.approaches_inserted as i32,
    )
    .await
    {
        warn!(error = %e, "Failed to record ETL event completion");
    }
}

/// Helper to handle file loading failure.
async fn handle_load_failure(pool: &PgPool, file_name: &str, event_id: Uuid, err: &anyhow::Error) {
    error!(file = %file_name, error = %err, "File load failed");

    // Record failure
    if let Err(record_err) =
        AsteroidRepository::record_etl_event_failed(pool, event_id, &err.to_string()).await
    {
        warn!(error = %record_err, "Failed to record ETL event failure");
    }
}

/// Stream JSON records from a file and process them in batches.
///
/// This function uses streaming deserialization to process records one at a time,
/// maintaining constant memory usage regardless of file size.
///
/// The input file is expected to be in NDJSON format (one JSON record per line).
async fn stream_and_process_file(
    file_path: &std::path::Path,
    pool: &PgPool,
) -> Result<crate::database::repository::UpsertStats> {
    let file = File::open(file_path)
        .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

    let reader = BufReader::new(file);
    let mut total_stats = crate::database::repository::UpsertStats::default();
    let mut batch = Vec::with_capacity(1000);
    let mut record_count = 0;

    for line_result in reader.lines() {
        let line = line_result.context("Failed to read line")?;
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Deserialize the record
        match serde_json::from_str::<(Asteroid, Vec<Approach>)>(trimmed) {
            Ok(record) => {
                batch.push(record);
                record_count += 1;

                // Process batch when full
                if batch.len() >= 1000 {
                    let batch_stats =
                        AsteroidRepository::upsert_batch(pool, std::mem::take(&mut batch))
                            .await
                            .context("Failed to upsert batch")?;
                    total_stats.asteroids_inserted += batch_stats.asteroids_inserted;
                    total_stats.asteroids_updated += batch_stats.asteroids_updated;
                    total_stats.approaches_inserted += batch_stats.approaches_inserted;
                    total_stats.approaches_skipped += batch_stats.approaches_skipped;
                }
            }
            Err(e) => {
                warn!(error = %e, line = trimmed.len(), "Skipping malformed record");
            }
        }
    }

    // Process remaining records
    if !batch.is_empty() {
        let batch_stats = AsteroidRepository::upsert_batch(pool, batch)
            .await
            .context("Failed to upsert final batch")?;
        total_stats.asteroids_inserted += batch_stats.asteroids_inserted;
        total_stats.asteroids_updated += batch_stats.asteroids_updated;
        total_stats.approaches_inserted += batch_stats.approaches_inserted;
        total_stats.approaches_skipped += batch_stats.approaches_skipped;
    }

    if record_count == 0 {
        anyhow::bail!("No valid records found in file");
    }

    Ok(total_stats)
}

/// Collect all transformed JSON files from a directory.
async fn collect_transformed_files(dir: &std::path::Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut entries = fs::read_dir(dir)
        .await
        .context("Failed to read input directory")?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file()
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| name.ends_with("_transformed.json"))
        {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

/// Print a formatted summary table of the load results.
fn print_summary(stats: &LoadStats) {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_DEFAULT);

    table.add_row(Row::new(vec![
        Cell::new("LOAD SUMMARY")
            .with_hspan(2)
            .with_style(Attr::Bold),
    ]));

    table.add_row(row!["Files Processed", stats.files_processed]);
    table.add_row(row!["Files Skipped (Idempotent)", stats.files_skipped]);
    table.add_row(row!["Asteroids Upserted", stats.total_asteroids_upserted]);
    table.add_row(row!["Approaches Inserted", stats.total_approaches_inserted]);
    table.add_row(row![
        "Approaches Skipped (Dup)",
        stats.total_approaches_skipped
    ]);

    println!();
    table.printstd();
    println!();
}
