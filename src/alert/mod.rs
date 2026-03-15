//! # Alerting System
//!
//! This module handles notifications and alerts for the Rustroid Sentinel system.
//! It includes integrations with messaging platforms like Discord and a core
//! alerting service for evaluating asteroid risk and triggering notifications.
//!
//! ## Submodules
//!
//! - [`discord`]: Discord webhook client for sending formatted alert embeds
//! - [`error`]: Error types for alert operations
//! - [`service`]: Core alert service that queries the database and dispatches notifications
//!
//! ## Alert Flow
//!
//! 1. [`AlertService`](service::AlertService) queries for hazardous approaches without alerts
//! 2. For each unalerted approach, a Discord notification is sent via [`DiscordClient`](discord::DiscordClient)
//! 3. Alert events are recorded to the database for idempotency
//!
//! ## Example
//!
//! ```rust,no_run
//! use rustroid_sentinel::alert::service::AlertService;
//! use rustroid_sentinel::alert::discord::DiscordClient;
//! use rustroid_sentinel::database::DatabasePool;
//! use rustroid_sentinel::settings::{RustroidSentinelConfig, DiscordConfig, EtlConfig};
//!
//! # async fn example(config: RustroidSentinelConfig) -> Result<(), anyhow::Error> {
//! let db = DatabasePool::new(&config.database).await?;
//! let discord = DiscordClient::new(config.discord.clone());
//! let service = AlertService::new(db, discord, config.etl.clone());
//!
//! service.check_and_send_alerts().await?;
//! # Ok(())
//! # }
//! ```

pub mod discord;
pub mod error;
pub mod service;
