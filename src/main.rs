//! # Rustroid Sentinel CLI
//!
//! This crate provides the command-line interface for the Rustroid Sentinel project.
//! It serves as the primary orchestration layer for running ETL pipelines,
//! starting the web server, and managing hazardous asteroid alerts.

use std::str::FromStr;

use clap::{Parser, Subcommand};
use rustroid_sentinel::{
    cli::alert::{self, AlertArgs},
    cli::extract::{self, ExtractArgs},
    cli::load::{self, LoadArgs},
    cli::orbits::{self, OrbitsArgs},
    cli::pipeline::{self, PipelineArgs},
    cli::prune::{self, PruneArgs},
    cli::report::{self, ReportArgs},
    cli::sentry::{self, SentryArgs},
    cli::transform::{self, TransformArgs},
    cli::vectorize::{self, VectorizeArgs},
    settings::RustroidSentinelConfig,
};
use tracing::{Level, error, info};

/// The primary entry point for the Rustroid Sentinel CLI.
///
/// Use subcommands to execute portions of the ETL pipeline, launch the web server,
/// or dispatch alerts.
#[derive(Parser)]
#[command(name = "rustroid-sentinel", author, version, about, long_about = None)]
struct Cli {
    /// The specific command to execute.
    #[command(subcommand)]
    command: Commands,
}

/// The available subcommands for the Rustroid Sentinel lifecycle.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Extracts asteroid data from NASA's NeoWs API.
    Extract(ExtractArgs),
    /// Transforms raw extracted data into domain models with hazard classification.
    Transform(TransformArgs),
    /// Loads transformed data into the PostgreSQL database.
    Load(LoadArgs),
    /// Checks for hazardous approaches and sends Discord alerts.
    #[cfg(feature = "alerting")]
    Alert(AlertArgs),
    /// Deletes stale rows per the configured retention policy.
    Prune(PruneArgs),
    /// Checks Sentry-flagged asteroids against JPL's Sentry API and stores
    /// real Torino/Palermo hazard scale values.
    Sentry(SentryArgs),
    /// Fetches orbital elements for asteroids from JPL's Small-Body Database API.
    Orbits(OrbitsArgs),
    /// Computes and stores pgvector similarity embeddings for every asteroid.
    Vectorize(VectorizeArgs),
    /// Aggregates the trailing week's approaches and sends a Discord report.
    #[cfg(feature = "alerting")]
    Report(ReportArgs),
    /// Runs extract, transform, load, prune, vectorize, and (Sundays) report
    /// in one process, so a compute-metered database wakes once per run.
    #[cfg(feature = "alerting")]
    Pipeline(PipelineArgs),
    /// Starts the high-performance web server.
    #[cfg(feature = "api")]
    Serve,
}

/// Main entry point for the binary.
///
/// Assembles the configuration from TOML files and environment variables,
/// sets up global tracing based on the configured log level, and dispatches the
/// requested CLI subcommand.
///
/// # Panics
///
/// This function will exit with status 1 rather than panicking for expected failures
/// (like missing config), but it may panic if the `tracing_subscriber` fails to
/// initialize or if system-level resources are unavailable.
#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() {
    // Load .env file into environment variables
    dotenvy::dotenv().ok();

    let settings = match RustroidSentinelConfig::new() {
        Ok(sentinel_config) => sentinel_config,
        Err(error) => {
            eprintln!("FATAL: Failed to load configuration: {}", error);
            std::process::exit(1);
        }
    };

    let log_level = Level::from_str(&settings.service.log_level).unwrap_or(Level::INFO);

    tracing_subscriber::fmt().with_max_level(log_level).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Extract(args) => {
            info!("Running 'extract' command");
            if let Err(e) = extract::execute(args, settings).await {
                error!(error = %e, "Extraction failed");
                std::process::exit(1);
            }
            info!("Extraction completed successfully");
        }
        Commands::Transform(args) => {
            info!("Running 'transform' command");
            if let Err(e) = transform::execute(args).await {
                error!(error = %e, "Transformation failed");
                std::process::exit(1);
            }
            info!("Transformation completed successfully");
        }
        Commands::Load(args) => {
            info!("Running 'load' command");
            if let Err(e) = load::execute(args, settings).await {
                error!(error = %e, "Load failed");
                std::process::exit(1);
            }
            info!("Load completed successfully");
        }
        #[cfg(feature = "alerting")]
        Commands::Alert(args) => {
            info!("Running 'alert' command");
            if let Err(e) = alert::execute(args, settings).await {
                error!(error = %e, "Alert failed");
                std::process::exit(1);
            }
            info!("Alert completed successfully");
        }
        Commands::Prune(args) => {
            info!("Running 'prune' command");
            if let Err(e) = prune::execute(args, settings).await {
                error!(error = %e, "Prune failed");
                std::process::exit(1);
            }
            info!("Prune completed successfully");
        }
        Commands::Sentry(args) => {
            info!("Running 'sentry' command");
            if let Err(e) = sentry::execute(args, settings).await {
                error!(error = %e, "Sentry check failed");
                std::process::exit(1);
            }
            info!("Sentry check completed successfully");
        }
        Commands::Orbits(args) => {
            info!("Running 'orbits' command");
            if let Err(e) = orbits::execute(args, settings).await {
                error!(error = %e, "Orbit check failed");
                std::process::exit(1);
            }
            info!("Orbit check completed successfully");
        }
        Commands::Vectorize(args) => {
            info!("Running 'vectorize' command");
            if let Err(e) = vectorize::execute(args, settings).await {
                error!(error = %e, "Vectorize failed");
                std::process::exit(1);
            }
            info!("Vectorize completed successfully");
        }
        #[cfg(feature = "alerting")]
        Commands::Report(args) => {
            info!("Running 'report' command");
            if let Err(e) = report::execute(args, settings).await {
                error!(error = %e, "Report failed");
                std::process::exit(1);
            }
            info!("Report completed successfully");
        }
        #[cfg(feature = "alerting")]
        Commands::Pipeline(args) => {
            info!("Running 'pipeline' command");
            if let Err(e) = pipeline::execute(args, settings).await {
                error!(error = %e, "Pipeline failed");
                std::process::exit(1);
            }
            info!("Pipeline completed successfully");
        }
        #[cfg(feature = "api")]
        Commands::Serve => {
            info!(
                target: "rustroid_sentinel::config",
                config = ?settings,
                "Service configuration loaded."
            );

            let _client =
                match rustroid_sentinel::api::client::SharedHttpClient::new(&settings).await {
                    Ok(client) => client,
                    Err(error) => {
                        error!(error = %error, "FATAL: Failed to create http client");
                        std::process::exit(1);
                    }
                };

            // Initialize database pool
            let db_pool =
                match rustroid_sentinel::database::DatabasePool::new(&settings.database).await {
                    Ok(pool) => pool,
                    Err(error) => {
                        error!(error = %error, "FATAL: Failed to create database pool");
                        std::process::exit(1);
                    }
                };

            info!(
                name = %settings.service.name,
                host = %settings.service.host,
                port = settings.service.port,
                env = %settings.service.env,
                "Starting service"
            );

            if let Err(error) = rustroid_sentinel::server::run_server(
                &settings.service,
                &settings.server,
                &settings.database,
                settings.prometheus.clone(),
                settings.grafana_cloud_prometheus.clone(),
                db_pool.pool().clone(),
                settings.service.version.clone(),
            )
            .await
            {
                error!(error = %error, "Server execution failed");
                std::process::exit(1);
            }
        }
    }
}
