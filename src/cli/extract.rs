//! The `extract` CLI subcommand.
//!
//! Responsible for querying the NASA NeoWs API over a specified date range
//! and persisting raw JSON responses to the local filesystem for processing.

// src/cli/extract.rs
use anyhow::{Context, Result};
use chrono::{Duration, NaiveDate, Utc};
use clap::Args;
use prettytable::{Attr, Cell, Row, Table, format, row};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

use crate::{
    api::client::SharedHttpClient, nasa::asteroid_neows::api::NeoWsApi,
    settings::RustroidSentinelConfig,
};

/// Arguments for the `extract` CLI subcommand.
#[derive(Args, Debug, Clone)]
pub struct ExtractArgs {
    /// Start date for extraction. Defaults to today's UTC date.
    #[arg(
        short,
        long,
        value_parser = clap::value_parser!(NaiveDate),
        default_value_t = Utc::now().date_naive()
    )]
    pub start_date: NaiveDate,

    /// End date for extraction. Defaults to tomorrow's UTC date.
    #[arg(
        short,
        long,
        value_parser = clap::value_parser!(NaiveDate),
        default_value_t = (Utc::now() + Duration::hours(24)).date_naive()
    )]
    pub end_date: NaiveDate,

    /// Output directory where raw JSON batch files will be saved.
    /// Defaults to "data/raw".
    #[arg(short, long, default_value = "data/raw")]
    pub output: Option<PathBuf>,

    /// Batch size in days per API request. Larger batches reduce API calls but
    /// may encounter rate limits or response size issues.
    #[arg(short, long, default_value = "7")]
    pub batch_size: Option<usize>,

    /// Force re-extraction of data even if corresponding files already exist on disk.
    #[arg(short, long)]
    pub force: bool,

    /// Dry run mode - Calculates the number of batches and API calls without
    /// actually making network requests or writing files.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExtractionMetadata {
    start_date: String,
    end_date: String,
    extracted_at: String,
    total_asteroids: usize,
    total_batches: usize,
    nasa_api_version: String,
}

/// Executes the extraction process.
///
/// Fetches potentially hazardous approach events from NASA NeoWs API
/// in daily batches, writing the raw responses to disk.
///
/// # Errors
/// Returns an error if directory creation fails, the API client fails
/// to initialize, or the date range is invalid.
#[allow(clippy::cognitive_complexity)]
pub async fn execute(args: ExtractArgs, settings: RustroidSentinelConfig) -> Result<()> {
    info!("Initializing extraction process");

    // Convert DateTime to NaiveDate for processing
    let start_date = args.start_date;
    let end_date = args.end_date;

    info!(start_date = %start_date, end_date = %end_date, "Extraction period defined");

    // Validate date range
    validate_date_range(&start_date, &end_date)?;

    // Create output directory
    let output_dir = args.output.unwrap_or_else(|| PathBuf::from("data/raw"));
    if !args.dry_run {
        fs::create_dir_all(&output_dir)
            .await
            .context("Failed to create output directory")?;
        info!(path = %output_dir.display(), "Output directory created");
    }

    let http_client = match SharedHttpClient::new(&settings).await {
        Ok(client) => client,
        Err(error) => {
            error!(error = %error, "FATAL: Failed to create http client");
            std::process::exit(1);
        }
    };

    // Initialize NASA API client
    let neo_ws_api_client = Arc::new(NeoWsApi::new(http_client, settings.nasa));

    if args.dry_run {
        info!("DRY RUN MODE - No API calls will be made");
        preview_extraction(&start_date, &end_date, args.batch_size.unwrap_or(7));
        return Ok(());
    }

    // Execute extraction in batches
    let batch_size = args.batch_size.unwrap_or(7);
    let extraction_result = extract_in_batches(
        neo_ws_api_client,
        &start_date,
        &end_date,
        batch_size,
        &output_dir,
        args.force,
    )
    .await?;

    // Save metadata
    save_metadata(&output_dir, &extraction_result).await?;

    // Print summary
    print_summary(&extraction_result);

    info!("Extraction completed successfully");
    Ok(())
}

/// Validate that date range is reasonable
fn validate_date_range(start: &NaiveDate, end: &NaiveDate) -> Result<()> {
    if start > end {
        anyhow::bail!("Start date cannot be after end date");
    }

    let days = (*end - *start).num_days();
    if days > 365 {
        warn!(
            days,
            "Date range spans many days. Consider breaking into smaller batches"
        );
    }

    if days == 0 {
        warn!("Start and end dates are identical. Extracting single day.");
    }

    Ok(())
}

/// Preview what would be extracted
fn preview_extraction(start: &NaiveDate, end: &NaiveDate, batch_size: usize) {
    let total_days = (*end - *start).num_days() + 1;
    let num_batches = (total_days as usize).div_ceil(batch_size);

    info!(
        "Extraction Preview: Total days: {}, Batch size: {} days, Number of batches: {}, Estimated API calls: {}",
        total_days, batch_size, num_batches, num_batches
    );
}

#[derive(Debug)]
struct ExtractionResult {
    start_date: NaiveDate,
    end_date: NaiveDate,
    total_asteroids: usize,
    total_batches: usize,
    successful_batches: usize,
    failed_batches: Vec<String>,
}

/// Extract data in batches
#[allow(clippy::too_many_lines)]
#[allow(clippy::ptr_arg)]
async fn extract_in_batches(
    client: Arc<NeoWsApi>,
    start_date: &NaiveDate,
    end_date: &NaiveDate,
    batch_size: usize,
    output_dir: &PathBuf,
    force: bool,
) -> Result<ExtractionResult> {
    let mut batches = Vec::new();
    let mut current_date = *start_date;
    let mut batch_num = 0;
    while current_date <= *end_date {
        batch_num += 1;
        let batch_end = std::cmp::min(
            current_date + chrono::Duration::days(batch_size as i64 - 1),
            *end_date,
        );
        batches.push((batch_num, current_date, batch_end));
        current_date = batch_end + chrono::Duration::days(1);
    }

    let semaphore = Arc::new(Semaphore::new(5));
    let mut join_handles = Vec::new();

    for (batch_num, batch_start, batch_end) in batches {
        let client = client.clone();
        let output_dir = output_dir.clone();
        let permit = semaphore.clone().acquire_owned().await?;

        join_handles.push(tokio::spawn(async move {
            let _permit = permit;

            let batch_file = output_dir.join(format!(
                "asteroids_{}_{}.json",
                batch_start.format("%Y%m%d"),
                batch_end.format("%Y%m%d")
            ));

            if !force && batch_file.exists() {
                debug!(
                    batch = batch_num,
                    start = %batch_start,
                    end = %batch_end,
                    "Batch skipped (already exists)"
                );
                return Ok(None);
            }

            debug!(
                batch = batch_num,
                start = %batch_start,
                end = %batch_end,
                "Extracting batch"
            );

            match client.get_feed(batch_start, batch_end).await {
                Ok(data) => {
                    let asteroid_count = data
                        .near_earth_objects
                        .values()
                        .map(|v| v.len())
                        .sum::<usize>();

                    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;

                    fs::write(&batch_file, json)
                        .await
                        .map_err(|e| e.to_string())?;

                    debug!(
                        batch = batch_num,
                        asteroid_count,
                        path = %batch_file.display(),
                        "Batch saved successfully"
                    );
                    Ok(Some(asteroid_count))
                }
                Err(e) => {
                    warn!(
                        batch = batch_num,
                        start = %batch_start,
                        end = %batch_end,
                        error = %e,
                        "Batch extraction failed"
                    );
                    Err(format!("{} to {}", batch_start, batch_end))
                }
            }
        }));
    }

    let mut total_asteroids = 0;
    let mut successful_batches = 0;
    let mut failed_batches = Vec::new();
    let total_batches = join_handles.len();

    for handle in join_handles {
        match handle.await? {
            Ok(Some(count)) => {
                total_asteroids += count;
                successful_batches += 1;
            }
            Ok(None) => {
                successful_batches += 1;
            }
            Err(batch_info) => {
                failed_batches.push(batch_info);
            }
        }
    }

    Ok(ExtractionResult {
        start_date: *start_date,
        end_date: *end_date,
        total_asteroids,
        total_batches,
        successful_batches,
        failed_batches,
    })
}

/// Save extraction metadata
async fn save_metadata(output_dir: &std::path::Path, result: &ExtractionResult) -> Result<()> {
    let metadata = ExtractionMetadata {
        start_date: result.start_date.to_string(),
        end_date: result.end_date.to_string(),
        extracted_at: Utc::now().to_rfc3339(),
        total_asteroids: result.total_asteroids,
        total_batches: result.total_batches,
        nasa_api_version: "v1".to_string(),
    };

    let metadata_file = output_dir.join("_metadata.json");
    let json = serde_json::to_string_pretty(&metadata)?;
    fs::write(&metadata_file, json).await?;

    debug!("Metadata saved to {}", metadata_file.display());
    Ok(())
}

/// Print extraction summary
fn print_summary(result: &ExtractionResult) {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_DEFAULT);
    table.add_row(Row::new(vec![
        Cell::new("EXTRACTION SUMMARY")
            .with_hspan(2)
            .with_style(Attr::Bold),
    ]));
    table.add_row(row![
        "Period",
        format!("{} to {}", result.start_date, result.end_date)
    ]);
    table.add_row(row!["Total Asteroids", result.total_asteroids]);
    table.add_row(row!["Total Batches", result.total_batches]);
    table.add_row(row!["Successful", result.successful_batches]);
    table.add_row(row!["Failed", result.failed_batches.len()]);

    println!();
    table.printstd();
    println!();

    if !result.failed_batches.is_empty() {
        warn!("Failed batches:");
        for batch in &result.failed_batches {
            warn!("  - {}", batch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_date_range_valid() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 10).unwrap();
        assert!(validate_date_range(&start, &end).is_ok());
    }

    #[test]
    fn test_validate_date_range_invalid() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 10).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        assert!(validate_date_range(&start, &end).is_err());
    }

    #[test]
    fn test_validate_date_range_single_day() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        assert!(validate_date_range(&start, &end).is_ok());
    }

    #[test]
    fn test_validate_date_range_large() {
        let start = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        // Spans many days, but shouldn't error, just warn
        assert!(validate_date_range(&start, &end).is_ok());
    }
}
