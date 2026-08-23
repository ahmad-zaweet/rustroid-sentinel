//! The `report` CLI subcommand.
//!
//! Aggregates the trailing 7 days of approaches and sends a weekly summary
//! embed to the configured Discord webhook. Normally invoked as the last
//! stage of `pipeline` (Sundays only); `--force` sends regardless of day.

use anyhow::Result;
use chrono::{Duration, Utc};
use clap::Args;
use prettytable::{Attr, Cell, Row, Table, format, row};
use tracing::info;

use crate::alert::discord::DiscordClient;
use crate::database::DatabasePool;
use crate::database::report::{ReportRepository, WeeklyReportSummary};
use crate::settings::RustroidSentinelConfig;

/// Arguments for the `report` CLI subcommand.
#[derive(Args, Debug, Default)]
pub struct ReportArgs {
    /// Print the summary without sending it to Discord.
    #[arg(long)]
    pub dry_run: bool,
}

/// Executes the report command.
///
/// # Errors
///
/// Returns an error if the database connection, migrations, or the
/// aggregation query fail.
pub async fn execute(args: ReportArgs, settings: RustroidSentinelConfig) -> Result<()> {
    let db_pool = DatabasePool::new(&settings.database).await?;
    db_pool.run_migrations().await?;

    let end_date = Utc::now().date_naive();
    let start_date = end_date - Duration::days(7);

    let summary =
        ReportRepository::get_weekly_summary(db_pool.pool(), start_date, end_date).await?;

    print_summary(&summary);

    if args.dry_run {
        info!("DRY RUN: weekly report not sent to Discord");
        return Ok(());
    }

    let discord = DiscordClient::new(settings.discord);
    discord.send_weekly_report(&summary).await?;

    info!(total = summary.total_approaches, "Weekly report sent");
    Ok(())
}

/// Print a formatted summary table of the weekly report.
fn print_summary(summary: &WeeklyReportSummary) {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_DEFAULT);

    table.add_row(Row::new(vec![
        Cell::new("WEEKLY REPORT SUMMARY")
            .with_hspan(2)
            .with_style(Attr::Bold),
    ]));

    table.add_row(row![
        "Period",
        format!("{} to {}", summary.start_date, summary.end_date)
    ]);
    table.add_row(row!["Total Approaches", summary.total_approaches]);
    table.add_row(row!["🔴 Critical", summary.critical_count]);
    table.add_row(row!["🟠 High", summary.high_count]);
    table.add_row(row!["🟡 Medium", summary.medium_count]);
    table.add_row(row!["🟢 Low", summary.low_count]);

    println!();
    table.printstd();
    println!();
}
