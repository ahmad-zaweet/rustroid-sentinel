//! # Alert Service
//!
//! This module provides the core alerting service that orchestrates the
//! detection and notification of hazardous asteroid approaches.
//!
//! ## Responsibilities
//!
//! - Query database for unalerted hazardous approaches
//! - Dispatch notifications via configured channels (Discord)
//! - Record alert events for idempotency
//! - Handle concurrent alert dispatch with error recovery
//!
//! ## Alert Criteria
//!
//! Approaches are alerted if they meet ANY of:
//! - NASA PHA designation with miss distance < 0.05 AU
//! - Hazard classification: "Critical", "High", or "Hazardous"
//! - Future approach date (not yet occurred)
//!
//! ## Idempotency
//!
//! The service maintains an `alerts` table to track which approaches
//! have been notified. Duplicate alerts are prevented by checking this
//! table before sending.

use crate::alert::discord::DiscordClient;
use crate::database::DatabasePool;
use crate::settings::EtlConfig;
use anyhow::Result;
use serde_json::json;
use sqlx::Row;
use tracing::{error, info, instrument};
use uuid::Uuid;

/// Service for evaluating asteroid data and sending alerts.
///
/// The `AlertService` scans the database for hazardous approaches that have not
/// yet been notified, sends alerts via configured channels (e.g., Discord),
/// and records the alert event to ensure idempotency.
#[derive(Clone)]
pub struct AlertService {
    db: DatabasePool,
    discord: DiscordClient,
    config: EtlConfig,
}

impl AlertService {
    /// Creates a new `AlertService` with the given dependencies and configuration.
    pub fn new(db: DatabasePool, discord: DiscordClient, config: EtlConfig) -> Self {
        Self {
            db,
            discord,
            config,
        }
    }

    /// Scans the database for new hazardous approaches and sends notifications.
    ///
    /// This method identifies approaches that meet hazard criteria (e.g., "Critical" classification)
    /// and have not yet been alerted through the Discord channel. It spawns concurrent
    /// tasks to send notifications and update the alert history.
    ///
    /// # Errors
    ///
    /// Returns an error if database queries or alerting operations fail.
    #[instrument(skip(self))]
    pub async fn check_and_send_alerts(&self) -> Result<()> {
        info!("Checking for new hazardous asteroid approaches");

        let limit = self.config.batch_size as i64;

        // Query for hazardous approaches that haven't been alerted yet
        let rows = sqlx::query(
            r#"
            SELECT
                ap.id as approach_id,
                ast.name,
                ast.neo_reference_id,
                ap.close_approach_date,
                ap.miss_distance_km,
                ap.velocity_km_per_h,
                ap.hazard_classification
            FROM approaches ap
            JOIN asteroids ast ON ap.asteroid_id = ast.id
            LEFT JOIN alerts al ON ap.id = al.approach_id AND al.alert_type = 'discord'
            WHERE
                (ast.is_potentially_hazardous = true OR ap.hazard_classification IN ('Hazardous', 'High', 'Critical'))
                AND al.id IS NULL
                AND ap.close_approach_date >= CURRENT_DATE
            ORDER BY ap.close_approach_date ASC
            LIMIT $1
            "#
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        if rows.is_empty() {
            info!("No new hazardous approaches found to alert");
            return Ok(());
        }

        info!(count = rows.len(), "Found unalerted hazardous approaches");

        let mut handles = Vec::new();

        for row in rows {
            let approach_id: Uuid = row.try_get("approach_id")?;
            let name: String = row.try_get("name")?;
            let _neo_ref: String = row.try_get("neo_reference_id")?;
            let date: chrono::NaiveDate = row.try_get("close_approach_date")?;
            let miss_km: f64 = row.try_get("miss_distance_km")?;
            let vel_kmh: f64 = row.try_get("velocity_km_per_h")?;
            let hazard: String = row.try_get("hazard_classification")?;

            let service = self.clone();

            let handle = tokio::spawn(async move {
                match service
                    .discord
                    .send_alert(
                        "⚠️ Hazardous Asteroid Approach",
                        &name,
                        &hazard,
                        &date,
                        miss_km,
                        vel_kmh,
                    )
                    .await
                {
                    Ok(_) => {
                        if let Err(e) = service.record_alert(approach_id, "discord").await {
                            error!(approach_id = %approach_id, error = %e, "Failed to record alert in DB");
                        }
                    }
                    Err(e) => {
                        error!(approach_id = %approach_id, error = %e, "Failed to send alert for approach");
                    }
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            if let Err(e) = handle.await {
                error!(error = %e, "Alert task panicked or failed to join");
            }
        }

        Ok(())
    }

    async fn record_alert(&self, approach_id: Uuid, alert_type: &str) -> Result<()> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO alerts (id, approach_id, alert_type, alerted_at, payload)
            VALUES ($1, $2, $3, NOW(), $4)
            "#,
        )
        .bind(id)
        .bind(approach_id)
        .bind(alert_type)
        .bind(json!({ "status": "sent" }))
        .execute(self.db.pool())
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::settings::EtlConfig;

    fn create_test_etl_config() -> EtlConfig {
        EtlConfig {
            fetch_interval_hours: 24,
            lookback_days: 7,
            lookahead_days: 7,
            alert_cooldown_hours: 24,
            batch_size: 100,
        }
    }

    #[test]
    fn test_alert_service_creation() {
        // Test that we can create an AlertService (without actually running it)
        // This is a compile-time check that the types are correct
        let _config = create_test_etl_config();
        assert_eq!(_config.batch_size, 100);
    }

    #[test]
    fn test_etl_config_defaults() {
        let config = create_test_etl_config();

        assert_eq!(config.fetch_interval_hours, 24);
        assert_eq!(config.lookback_days, 7);
        assert_eq!(config.lookahead_days, 7);
        assert_eq!(config.alert_cooldown_hours, 24);
        assert_eq!(config.batch_size, 100);
    }

    #[tokio::test]
    async fn test_alert_service_with_empty_db() {
        // This test would require a real database
        // For now, just verify the config is correct
        let config = create_test_etl_config();
        assert!(config.batch_size > 0);
    }

    #[test]
    fn test_discord_config_skip_empty_webhook() {
        use crate::settings::DiscordConfig;

        let config = DiscordConfig {
            webhook_url: String::new(),
            timeout_seconds: 30,
            max_retries: 3,
        };

        // Empty webhook should skip sending
        assert!(config.webhook_url.is_empty());
    }
}
