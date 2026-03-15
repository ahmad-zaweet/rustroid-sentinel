//! # Transform CLI Command
//!
//! This module implements the `transform` subcommand, which processes raw NASA API
//! responses into internal domain models with hazard classification.

// src/cli/transform.rs
use anyhow::{Context, Result};
use clap::Args;
use prettytable::{Attr, Cell, Row, Table, format, row};
use std::path::PathBuf;
use tokio::fs;
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};

use crate::models::HazardClassification;
use crate::nasa::asteroid_neows::responses::NeoFeed;
use crate::transform::transform_neo_feed;

/// Arguments for the `transform` CLI subcommand.
///
/// The transform command converts raw NASA JSON data into enriched domain models
/// with hazard classifications, ready for database loading.
///
/// # Examples
///
/// ```bash
/// # Transform all files in default directory
/// rustroid-sentinel transform
///
/// # Transform with custom paths
/// rustroid-sentinel transform -i /data/raw -o /data/transformed
///
/// # Force re-transformation
/// rustroid-sentinel transform --force
///
/// # Preview without writing (dry run)
/// rustroid-sentinel transform --dry-run
/// ```
#[derive(Args, Debug, Clone)]
pub struct TransformArgs {
    /// Input directory containing raw JSON batch files from the extraction stage.
    #[arg(short, long, default_value = "data/raw")]
    pub input: PathBuf,

    /// Output directory where transformed domain model JSON files will be saved.
    #[arg(short, long, default_value = "data/transformed")]
    pub output: PathBuf,

    /// Force re-transformation of data even if output files already exist.
    #[arg(short, long)]
    pub force: bool,

    /// Dry run mode - Identifies files to transform and previews output paths
    /// without writing any data.
    #[arg(long)]
    pub dry_run: bool,
}

/// Statistics collected during the transformation process.
#[derive(Debug, Default)]
struct TransformStats {
    files_processed: usize,
    files_skipped: usize,
    total_asteroids: usize,
    total_approaches: usize,
    critical_count: usize,
    high_count: usize,
    medium_count: usize,
    low_count: usize,
}

/// Executes the transformation process.
///
/// This command processes raw NASA API responses into our internal domain models.
/// It performs the following steps:
/// 1. Collects all `.json` files from the input directory.
/// 2. Spawns concurrent tasks to process each file using `transform_neo_feed`.
/// 3. Classifies hazards for each asteroid approach based on domestic criteria.
/// 4. Writes the enriched domain models to the output directory.
///
/// # Errors
///
/// Returns an error if directory creation fails, or if a specific file
/// cannot be read, parsed, or transformed.
#[allow(clippy::cognitive_complexity)]
#[allow(clippy::too_many_lines)]
pub async fn execute(args: TransformArgs) -> Result<()> {
    info!("Starting transform command");

    // Validate input directory
    if !args.input.exists() {
        anyhow::bail!(
            "Input directory '{}' does not exist. Run 'extract' first.",
            args.input.display()
        );
    }

    // Create output directory
    if !args.dry_run {
        fs::create_dir_all(&args.output)
            .await
            .context("Failed to create output directory")?;
    }

    // Collect JSON files (excluding metadata)
    let json_files = collect_json_files(&args.input).await?;

    if json_files.is_empty() {
        warn!("No JSON files found in '{}'", args.input.display());
        return Ok(());
    }

    info!(count = json_files.len(), "Found JSON files to transform");

    if args.dry_run {
        info!("DRY RUN MODE - No files will be written");
        for file in &json_files {
            info!(path = %file.display(), "Dry run: Would transform file");
        }
        return Ok(());
    }

    let mut set = JoinSet::new();

    for file_path in json_files {
        let args = args.clone();
        set.spawn(async move {
            let mut local_stats = TransformStats::default();
            let file_name = file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let output_file = args.output.join(format!("{}_transformed.json", file_name));

            // Skip if already transformed (unless forced)
            if !args.force && output_file.exists() {
                debug!(file = %file_name, "Skipping file (already transformed)");
                local_stats.files_skipped += 1;
                return Ok(local_stats);
            }

            info!(path = %file_path.display(), "Transforming raw data");

            // Read and deserialize the raw JSON
            let raw_json = fs::read_to_string(&file_path)
                .await
                .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

            let feed: NeoFeed = serde_json::from_str(&raw_json)
                .with_context(|| format!("Failed to parse JSON in: {}", file_path.display()))?;

            // Transform
            let transformed = transform_neo_feed(&feed);

            // Collect statistics
            for (_, approaches) in &transformed {
                local_stats.total_approaches += approaches.len();
                for approach in approaches {
                    match approach.hazard_classification {
                        HazardClassification::Critical => local_stats.critical_count += 1,
                        HazardClassification::High => local_stats.high_count += 1,
                        HazardClassification::Medium => local_stats.medium_count += 1,
                        HazardClassification::Low => local_stats.low_count += 1,
                    }
                }
            }
            local_stats.total_asteroids += transformed.len();
            local_stats.files_processed += 1;

            // Serialize and write output in NDJSON format (one record per line)
            // This enables streaming during the load phase for constant memory usage
            let mut output = String::new();
            for record in &transformed {
                let line = serde_json::to_string(record)
                    .context("Failed to serialize transformed data")?;
                output.push_str(&line);
                output.push('\n');
            }

            fs::write(&output_file, output)
                .await
                .with_context(|| format!("Failed to write output: {}", output_file.display()))?;

            debug!(
                file = %file_name,
                output = %output_file.display(),
                asteroid_count = transformed.len(),
                "File transformed successfully"
            );

            Ok::<TransformStats, anyhow::Error>(local_stats)
        });
    }

    let mut stats = TransformStats::default();
    while let Some(res) = set.join_next().await {
        match res {
            Ok(Ok(local_stats)) => {
                stats.files_processed += local_stats.files_processed;
                stats.files_skipped += local_stats.files_skipped;
                stats.total_asteroids += local_stats.total_asteroids;
                stats.total_approaches += local_stats.total_approaches;
                stats.critical_count += local_stats.critical_count;
                stats.high_count += local_stats.high_count;
                stats.medium_count += local_stats.medium_count;
                stats.low_count += local_stats.low_count;
            }
            Ok(Err(e)) => {
                error!(error = %e, "Failed to transform a file");
                return Err(e);
            }
            Err(e) => {
                anyhow::bail!("Join error while transforming files: {}", e);
            }
        }
    }

    print_summary(&stats);
    info!("Transformation completed successfully");
    Ok(())
}

/// Collect all JSON files from a directory, excluding metadata files.
async fn collect_json_files(dir: &std::path::Path) -> Result<Vec<PathBuf>> {
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
                .is_some_and(|name| name.ends_with(".json") && !name.starts_with('_'))
        {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

/// Print a formatted summary table of the transformation results.
fn print_summary(stats: &TransformStats) {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_DEFAULT);

    table.add_row(Row::new(vec![
        Cell::new("TRANSFORMATION SUMMARY")
            .with_hspan(2)
            .with_style(Attr::Bold),
    ]));

    table.add_row(row!["Files Processed", stats.files_processed]);
    table.add_row(row!["Files Skipped", stats.files_skipped]);
    table.add_row(row!["Total Asteroids", stats.total_asteroids]);
    table.add_row(row!["Total Approaches", stats.total_approaches]);
    table.add_row(row!["", ""]);
    table.add_row(Row::new(vec![
        Cell::new("HAZARD BREAKDOWN")
            .with_hspan(2)
            .with_style(Attr::Bold),
    ]));
    table.add_row(row!["🔴 Critical", stats.critical_count]);
    table.add_row(row!["🟠 High", stats.high_count]);
    table.add_row(row!["🟡 Medium", stats.medium_count]);
    table.add_row(row!["🟢 Low", stats.low_count]);

    println!();
    table.printstd();
    println!();
}
