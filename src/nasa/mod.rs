//! # NASA API Integration
//!
//! This module provides a high-level client and data structures for interacting
//! with various NASA Open APIs. Currently, it primarily focuses on the NeoWs
//! (Near Earth Object Web Service).
//!
//! ## Submodules
//!
//! - [`asteroid_neows`]: Client and types for the NeoWs API
//! - [`jpl_sentry`]: Client and types for the JPL Sentry impact-monitoring API
//! - [`jpl_sbdb`]: Client and types for the JPL Small-Body Database API
//! - [`error`]: Error types specific to NASA API interactions
//!
//! ## Supported APIs
//!
//! | API | Description | Status |
//! |-----|-------------|--------|
//! | NeoWs | Near-Earth Object data | ✅ Implemented |
//! | Sentry | Impact probability / Torino / Palermo scales | ✅ Implemented |
//! | SBDB | Orbital elements, orbit class, spectral class, albedo | ✅ Implemented |
//!
//! ## Example
//!
//! ```rust,no_run
//! use rustroid_sentinel::nasa::asteroid_neows::api::NeoWsApi;
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // See asteroid_neows module for full example
//! # Ok(())
//! # }
//! ```

pub mod asteroid_neows;
pub mod error;
pub mod jpl_sbdb;
pub mod jpl_sentry;
