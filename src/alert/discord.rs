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
//! }).await?;
//! # Ok(())
//! # }
//! ```

use crate::settings::DiscordConfig;
use serenity::builder::{CreateEmbed, ExecuteWebhook};
use serenity::http::Http;
use serenity::model::colour::Colour;
use serenity::model::webhook::Webhook;
use std::sync::Arc;
use tracing::{info, warn};

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
    ) -> Result<(), serenity::Error> {
        if self.config.webhook_url.is_empty() {
            warn!("Discord webhook URL is empty, skipping alert.");
            return Ok(());
        }

        let webhook = Webhook::from_url(&self.http, &self.config.webhook_url).await?;

        // Modern Hex Color Mapping
        let color = match alert.hazard {
            "Critical" => Colour::from_rgb(255, 69, 58), // System Red
            "High" => Colour::from_rgb(255, 159, 10),    // System Orange
            "Medium" => Colour::from_rgb(255, 214, 10),  // System Yellow
            _ => Colour::from_rgb(48, 209, 88),          // System Green
        };

        // Format numbers for readability
        let miss_str = if alert.miss_distance_km >= 1_000_000.0 {
            format!("{:.2}M km", alert.miss_distance_km / 1_000_000.0)
        } else {
            format!("{:.0} km", alert.miss_distance_km)
        };

        let vel_str = format!("{:.0} km/h", alert.velocity_km_h);
        let diameter_str = format!("{:.3} km", alert.diameter_avg_km);

        let embed = CreateEmbed::new()
            .title(format!("{} : {}", alert.title, alert.asteroid_name))
            .description(format!(
                "🎯 **Target Identification:** `{}`",
                alert.asteroid_name
            ))
            .color(color)
            .thumbnail("https://img.icons8.com/isometric/512/asteroid.png")
            .field("📡 Status", format!("**{}**", alert.hazard), true)
            .field("📅 Approach Date", format!("`{}`", alert.date), true)
            .field("\u{200B}", "\u{200B}", true) // Spacer for 3-column layout
            .field("📏 Miss Distance", format!("`{}`", miss_str), true)
            .field("🚀 Velocity", format!("`{}`", vel_str), true)
            .field("🪨 Est. Diameter", format!("`{}`", diameter_str), true)
            .footer(
                serenity::builder::CreateEmbedFooter::new("Rustroid Sentinel")
                    .icon_url("https://img.icons8.com/color/48/shield.png"),
            )
            .timestamp(
                serenity::model::Timestamp::from_unix_timestamp(chrono::Utc::now().timestamp())
                    .unwrap_or_else(|_| serenity::model::Timestamp::default()),
            );

        let builder = ExecuteWebhook::new()
            .username("Rustroid Sentinel Central")
            .avatar_url("https://img.icons8.com/color/96/artificial-intelligence.png")
            .content(if alert.hazard == "Critical" || alert.hazard == "High" {
                "🚨 **HIGH ALERT: Potential Impact Risk Detected** 🚨"
            } else {
                "ℹ️ **Observation: New Near-Earth Object approach recorded**"
            })
            .embeds(vec![embed]);

        webhook.execute(&self.http, false, builder).await?;

        info!("Sent modernized Discord alert for {}", alert.asteroid_name);

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
            })
            .await;

        assert!(
            result.is_ok(),
            "Should return OK without sending if webhook is empty"
        );
    }
}
