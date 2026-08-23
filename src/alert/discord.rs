//! # Discord Alert Integration
//!
//! This module provides a Discord webhook client for sending formatted asteroid
//! alert notifications. It uses the [`serenity`] library to construct rich embeds
//! with color-coded hazard levels and numerical formatting.
//!
//! ## Features
//!
//! - **Rich Embeds**: Formatted alerts with color coding (Critical=Red, High=Orange, etc.)
//! - **Idempotency**: Skips sending if webhook URL is empty
//! - **Error Handling**: Logs warnings and returns gracefully on failures
//!
//! ## Example
//!
//! ```rust,no_run
//! use rustroid_sentinel::alert::discord::{AsteroidApproachAlert, DiscordClient};
//! use rustroid_sentinel::settings::DiscordConfig;
//! use chrono::NaiveDate;
//!
//! # async fn example() -> Result<(), serenity::Error> {
//! let config = DiscordConfig {
//!     webhook_url: "https://discord.com/api/webhooks/...".to_string(),
//!     timeout_seconds: 30,
//!     max_retries: 3,
//! };
//!
//! let client = DiscordClient::new(config);
//! let date = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();
//!
//! client.send_alert(AsteroidApproachAlert {
//!     title: "Hazardous Asteroid",
//!     asteroid_name: "2024 AB12",
//!     hazard: "High",
//!     date: &date,
//!     miss_distance_km: 150_000.0,
//!     velocity_km_h: 72_000.0,
//!     diameter_avg_km: 0.75,
//!     torino_scale: None,
//!     palermo_scale: None,
//! }).await?;
//! # Ok(())
//! # }
//! ```

use crate::api::handlers::dashboard::format_number;
use crate::database::report::WeeklyReportSummary;
use crate::settings::DiscordConfig;
use serenity::builder::{CreateEmbed, ExecuteWebhook};
use serenity::http::Http;
use serenity::model::colour::Colour;
use serenity::model::webhook::Webhook;
use std::sync::Arc;
use tracing::{info, warn};

/// Kilometers per astronomical unit.
const KM_PER_AU: f64 = 149_597_870.7;

/// Formats a number with thousands separators (e.g. `144832.0` -> `144,832`).
fn format_commas(num: f64) -> String {
    let rounded = num.round() as i64;
    let digits = rounded.abs().to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let grouped: String = grouped.chars().rev().collect();
    if rounded < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

/// Client for interacting with Discord webhooks.
///
/// This client uses the `serenity` library to send formatted alerts and embeds
/// to a configured Discord webhook URL.
#[derive(Clone)]
pub struct DiscordClient {
    http: Arc<Http>,
    config: DiscordConfig,
}

/// Data describing a single asteroid close-approach alert.
pub struct AsteroidApproachAlert<'a> {
    /// The alert title (e.g., "Hazardous Asteroid Approach").
    pub title: &'a str,
    /// The name of the asteroid.
    pub asteroid_name: &'a str,
    /// The hazard classification string.
    pub hazard: &'a str,
    /// The close approach date.
    pub date: &'a chrono::NaiveDate,
    /// The miss distance in kilometers.
    pub miss_distance_km: f64,
    /// The relative velocity in km/h.
    pub velocity_km_h: f64,
    /// The average (midpoint) estimated diameter in kilometers.
    pub diameter_avg_km: f64,
    /// JPL Sentry Torino Scale (0-10), if this asteroid is a currently
    /// tracked virtual impactor.
    pub torino_scale: Option<i16>,
    /// JPL Sentry cumulative Palermo Scale, if this asteroid is a currently
    /// tracked virtual impactor.
    pub palermo_scale: Option<f64>,
}

impl DiscordClient {
    /// Creates a new `DiscordClient` from the provided configuration.
    pub fn new(config: DiscordConfig) -> Self {
        Self {
            http: Arc::new(Http::new("")),
            config,
        }
    }

    /// Sends an alert notification to the configured Discord webhook.
    ///
    /// Formats the asteroid data into an embed with color coding based on
    /// hazard level (Critical: Red, High: Orange, Default: Gold).
    ///
    /// # Arguments
    ///
    /// * `title` - The alert title (e.g., "Hazardous Asteroid Approach").
    /// * `asteroid_name` - The name of the asteroid.
    /// * `hazard` - The hazard classification string.
    /// * `details` - Additional approach details for the embed fields.
    ///
    /// # Errors
    ///
    /// Returns a `serenity::Error` if the webhook execution fails.
    /// Sends a modernized alert notification to the configured Discord webhook.
    ///
    /// Formats the asteroid data into a premium embed with color coding,
    /// categorized fields, and high-fidelity numerical formatting.
    ///
    /// # Arguments
    ///
    /// * `alert` - The asteroid approach data to format into the embed.
    ///
    /// # Errors
    ///
    /// Returns a `serenity::Error` if the webhook execution fails.
    pub async fn send_alert(
        &self,
        alert: AsteroidApproachAlert<'_>,
    ) -> Result<(), Box<serenity::Error>> {
        if self.config.webhook_url.is_empty() {
            warn!("Discord webhook URL is empty, skipping alert.");
            return Ok(());
        }

        let webhook = Webhook::from_url(&self.http, &self.config.webhook_url).await?;

        // Hazard Token Hex Color Mapping
        let color = match alert.hazard {
            "Critical" => Colour::from_rgb(239, 68, 68), // #EF4444 hazard-critical
            "High" => Colour::from_rgb(249, 115, 22),    // #F97316 hazard-high
            "Medium" => Colour::from_rgb(234, 179, 8),   // #EAB308 hazard-medium
            _ => Colour::from_rgb(34, 197, 94),          // #22C55E hazard-low
        };

        // Format numbers for readability
        let miss_str = if alert.miss_distance_km >= 1_000_000.0 {
            format!("{:.2}M km", alert.miss_distance_km / 1_000_000.0)
        } else {
            format!("{:.0} km", alert.miss_distance_km)
        };

        let vel_str = format!("{} km/h", format_commas(alert.velocity_km_h));
        let diameter_str = format!("{:.3} km", alert.diameter_avg_km);
        let au = alert.miss_distance_km / KM_PER_AU;
        let torino_str = alert
            .torino_scale
            .map(|t| t.to_string())
            .unwrap_or_else(|| "—".to_string());

        let mut embed = CreateEmbed::new()
            .title(format!("{}: {}", alert.title, alert.asteroid_name))
            .description(format!(
                "Target crosses inside {au:.2} AU on {}.",
                alert.date
            ))
            .color(color)
            .thumbnail("https://rustroid-sentinel.onrender.com/img/logo-mark.png")
            .field("Status", alert.hazard, true)
            .field("Approach", format!("{}", alert.date), true)
            .field("Torino", torino_str, true)
            .field("Miss distance", miss_str, true)
            .field("Velocity", vel_str, true)
            .field("Diameter", diameter_str, true);

        // Only PHA/Sentry-flagged objects that are currently tracked virtual
        // impactors have a Palermo scale; keep it as a trailing field so it
        // doesn't disturb the 3-column grid above.
        if let Some(palermo) = alert.palermo_scale {
            embed = embed.field("Palermo Scale", format!("{palermo:.2}"), false);
        }

        let embed = embed
            .footer(
                serenity::builder::CreateEmbedFooter::new("Rustroid Sentinel")
                    .icon_url("https://rustroid-sentinel.onrender.com/img/favicon-32x32.png"),
            )
            .timestamp(
                serenity::model::Timestamp::from_unix_timestamp(chrono::Utc::now().timestamp())
                    .unwrap_or_else(|_| serenity::model::Timestamp::default()),
            );

        let builder = ExecuteWebhook::new()
            .username("Rustroid Sentinel Central")
            .avatar_url("https://rustroid-sentinel.onrender.com/img/logo-mark.png")
            .content(if alert.hazard == "Critical" || alert.hazard == "High" {
                "Critical approach detected"
            } else {
                "New asteroid approach recorded"
            })
            .embeds(vec![embed]);

        webhook.execute(&self.http, false, builder).await?;

        info!("Sent modernized Discord alert for {}", alert.asteroid_name);

        Ok(())
    }

    /// Sends the weekly summary report to the configured Discord webhook.
    ///
    /// # Errors
    ///
    /// Returns a `serenity::Error` if the webhook execution fails.
    pub async fn send_weekly_report(
        &self,
        summary: &WeeklyReportSummary,
    ) -> Result<(), Box<serenity::Error>> {
        if self.config.webhook_url.is_empty() {
            warn!("Discord webhook URL is empty, skipping weekly report.");
            return Ok(());
        }

        let webhook = Webhook::from_url(&self.http, &self.config.webhook_url).await?;

        let mut embed = CreateEmbed::new()
            .title(format!(
                "Weekly Report · {} → {}",
                summary.start_date.format("%b %d"),
                summary.end_date.format("%b %d")
            ))
            .description(format!(
                "{} close approaches recorded this week.",
                summary.total_approaches
            ))
            .color(Colour::from_rgb(124, 58, 237)) // #7C3AED nebula-purple
            .thumbnail("https://rustroid-sentinel.onrender.com/img/logo-mark.png")
            .field("Total", format!("{}", summary.total_approaches), true)
            .field(
                "Critical / High",
                format!("{} / {}", summary.critical_count, summary.high_count),
                true,
            )
            .field(
                "Medium / Low",
                format!("{} / {}", summary.medium_count, summary.low_count),
                true,
            );

        if summary.closest_approach.is_some()
            || summary.fastest_approach.is_some()
            || summary.largest_asteroid.is_some()
        {
            if let Some(closest) = &summary.closest_approach {
                embed = embed.field(
                    "Closest",
                    format!("{} km", format_number(closest.miss_distance_km)),
                    true,
                );
            }

            if let Some(fastest) = &summary.fastest_approach {
                embed = embed.field(
                    "Fastest",
                    format!("{} km/h", format_commas(fastest.velocity_km_per_h)),
                    true,
                );
            }

            if let Some(largest) = &summary.largest_asteroid {
                embed = embed.field(
                    "Largest",
                    format!("{:.3} km", largest.estimated_diameter_avg_km),
                    true,
                );
            }
        }

        let embed = embed
            .footer(
                serenity::builder::CreateEmbedFooter::new("Rustroid Sentinel")
                    .icon_url("https://rustroid-sentinel.onrender.com/img/favicon-32x32.png"),
            )
            .timestamp(
                serenity::model::Timestamp::from_unix_timestamp(chrono::Utc::now().timestamp())
                    .unwrap_or_else(|_| serenity::model::Timestamp::default()),
            );

        let builder = ExecuteWebhook::new()
            .username("Rustroid Sentinel Central")
            .avatar_url("https://rustroid-sentinel.onrender.com/img/logo-mark.png")
            .content("Weekly asteroid activity report")
            .embeds(vec![embed]);

        webhook.execute(&self.http, false, builder).await?;

        info!(
            total = summary.total_approaches,
            "Sent weekly Discord report"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discord_client_skip_empty_webhook() {
        let config = DiscordConfig {
            webhook_url: "".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
        };

        let client = DiscordClient::new(config);
        let date = chrono::NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();

        let result = client
            .send_alert(AsteroidApproachAlert {
                title: "Test",
                asteroid_name: "Asteroid",
                hazard: "High",
                date: &date,
                miss_distance_km: 100.0,
                velocity_km_h: 100.0,
                diameter_avg_km: 0.5,
                torino_scale: None,
                palermo_scale: None,
            })
            .await;

        assert!(
            result.is_ok(),
            "Should return OK without sending if webhook is empty"
        );
    }
}
