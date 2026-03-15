//! # NASA API Integration
//!
//! This module provides a high-level client and data structures for interacting
//! with various NASA Open APIs. Currently, it primarily focuses on the NeoWs
//! (Near Earth Object Web Service).
//!
//! ## Submodules
//!
//! - [`asteroid_neows`]: Client and types for the NeoWs API
//! - [`error`]: Error types specific to NASA API interactions
//!
//! ## Supported APIs
//!
//! | API | Description | Status |
//! |-----|-------------|--------|
//! | NeoWs | Near-Earth Object data | ✅ Implemented |
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
