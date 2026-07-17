//! Example demonstrating how to dispatch a custom Discord Alert directly.
//!
//! `cargo run --example custom_alert`

use rustroid_sentinel::alert::discord::{AsteroidApproachAlert, DiscordClient};
use rustroid_sentinel::settings::RustroidSentinelConfig;

#[tokio::main]
async fn main() {
    let config = RustroidSentinelConfig::new().expect("Failed to load config");

    // Fallback if no webhook is configured locally
    if config.discord.webhook_url.is_empty() {
        println!("No Discord WEBHOOK_URL configured. Exiting.");
        return;
    }

    let discord_client = DiscordClient::new(config.discord);

    let asteroid_name = "Mock-Asteroid-9000";
    let hazard_level = "High";
    let date = chrono::NaiveDate::from_ymd_opt(2024, 12, 1).unwrap();
    let miss_distance_km = 1_200_000.0;
    let velocity_km_h = 88_000.0;
    let diameter_avg_km = 0.75;

    println!("Attempting to send an alert for {}...", asteroid_name);

    match discord_client
        .send_alert(AsteroidApproachAlert {
            title: "⚠️ Hazardous Asteroid Approach",
            asteroid_name,
            hazard: hazard_level,
            date: &date,
            miss_distance_km,
            velocity_km_h,
            diameter_avg_km,
        })
        .await
    {
        Ok(_) => println!("Successfully mocked Discord alert dispatch!"),
        Err(e) => println!("Failed to send alert: {}", e),
    }
}
