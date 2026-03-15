#![deny(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

//! # Rustroid Sentinel
//!
//! `rustroid-sentinel` is a high-performance system designed to monitor near-Earth objects (NEOs).
//! By interfacing with the NASA NeoWs API, it constructs an automated ETL pipeline that identifies,
//! stores, and tracks potentially hazardous asteroids.
//!
//! ## Core Features
//! - **Automated ETL**: Scalable pipeline for extracting and transforming NASA NeoWs data.
//! - **Hazard Detection**: Advanced classification logic for identifying potentially hazardous objects.
//! - **Real-time API**: High-performance REST API with built-in response caching and rate limiting.
//! - **Observability**: First-class support for Prometheus metrics and OpenTelemetry tracing.
//! - **Alerting**: Pluggable notification system for critical hazard alerts.
//!
//! ## Quick Start
//! ```rust,no_run
//! use rustroid_sentinel::settings::RustroidSentinelConfig;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Load configuration from config/config.toml and environment variables
//!     let config = RustroidSentinelConfig::new()?;
//!     
//!     println!("Initializing {} v{}", config.service.name, config.service.version);
//!     
//!     // Application logic goes here...
//!     Ok(())
//! }
//! ```
//!
//! ## Architecture
//! The library is split into several domain-specific modules:
//! - **NASA API Client** ([`nasa`]): Handles HTTP communication, deserialization, and rate-limiting
//!   for the external NeoWs endpoints.
//! - **Database & Storage** ([`database`], [`models`]): Manages the PostgreSQL connection pool and
//!   provides strict, typed queries for persisting pipeline data.
//! - **Web Server** ([`server`], [`api`]): Hosts the asynchronous REST API and static dashboard
//!   interface using `axum`.
//! - **Alerting** ([`alert`]): Background job processors that dispatch webhook notifications
//!   for hazardous approaches.
//! - **CLI** ([`cli`]): Command-line interfaces to run the application components individually.
//!
//! ## Operational Excellence
//! The library is built with production tracing ([`tracing`]) and Prometheus
//! metrics ([`metrics`]) baked into all critical paths.

#[cfg(feature = "alerting")]
pub mod alert;
#[cfg(feature = "api")]
pub mod api;
pub mod cli;
pub mod database;
pub mod error;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod models;
pub mod nasa;
#[cfg(feature = "api")]
pub mod server;
pub mod settings;
pub mod transform;
