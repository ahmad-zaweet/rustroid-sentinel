//! # Alert CLI Command
//!
//! This module implements the `alert` subcommand, which checks for new hazardous
//! asteroid approaches and sends notifications via configured channels (Discord).

use crate::alert::discord::DiscordClient;
use crate::alert::service::AlertService;
use crate::database::DatabasePool;
use crate::settings::RustroidSentinelConfig;
use anyhow::Result;
use clap::Args;

/// Arguments for the `alert` CLI subcommand.
///
/// The alert command currently accepts no arguments and uses configuration
/// from the settings file.
///
/// # Examples
///
/// ```bash
/// # Run alert check
/// rustroid-sentinel alert
/// ```
#[derive(Args, Debug)]
pub struct AlertArgs {}

/// Executes the alert command.
///
/// This function:
/// 1. Initializes the database connection pool
/// 2. Runs database migrations
/// 3. Creates the Discord client and alert service
/// 4. Checks for unalerted hazardous approaches and sends notifications
///
/// # Arguments
///
/// * `_args` - Command arguments (currently unused)
/// * `settings` - Application configuration
///
/// # Errors
///
/// Returns an error if:
/// - Database connection fails
/// - Migrations fail
/// - Alert service encounters an error
///
/// # Examples
///
/// ```rust,no_run
/// # use rustroid_sentinel::settings::RustroidSentinelConfig;
/// # use rustroid_sentinel::cli::alert::{AlertArgs, execute};
/// # async fn example() -> Result<(), anyhow::Error> {
/// let settings = RustroidSentinelConfig::new()?;
/// let args = AlertArgs {};
/// execute(args, settings).await?;
/// # Ok(())
/// # }
/// ```
pub async fn execute(_args: AlertArgs, settings: RustroidSentinelConfig) -> Result<()> {
    let db = DatabasePool::new(&settings.database).await?;
    db.run_migrations().await?;

    let discord = DiscordClient::new(settings.discord.clone());
    let service = AlertService::new(db, discord, settings.etl.clone());

    service.check_and_send_alerts().await?;

    Ok(())
}
