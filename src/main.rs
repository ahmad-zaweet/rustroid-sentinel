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
    cli::transform::{self, TransformArgs},
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

            // Run migrations
            if let Err(error) = db_pool.run_migrations().await {
                error!(error = %error, "FATAL: Failed to run database migrations");
                std::process::exit(1);
            }

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
