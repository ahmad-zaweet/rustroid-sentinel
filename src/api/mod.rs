//! # HTTP API Layer
//!
//! This module provides the REST API for the Rustroid Sentinel system, built with [`axum`].
//! It exposes endpoints for querying asteroid data, monitoring system health, and visualizing
//! approach statistics.
//!
//! ## Architecture
//!
//! The API layer is organized into the following submodules:
//!
//! - [`client`]: HTTP client utilities for external API communication
//! - [`error`]: Error types for API operations
//! - [`handlers`]: Request handlers implementing business logic for each endpoint
//! - [`routes`]: Router configuration defining the API endpoint structure
//! - [`templates`]: Server-side rendered HTML templates for the dashboard UI
//! - [`types`]: Shared types for API requests, responses, and pagination
//!
//! ## Endpoints
//!
//! | Method | Path              | Description                              |
//! |--------|-------------------|------------------------------------------|
//! | GET    | `/`               | Dashboard HTML page (SSR)                |
//! | GET    | `/api/health`     | Health check with database connectivity  |
//! | GET    | `/api/stats`      | Dashboard statistics summary             |
//! | GET    | `/api/velocity`   | Velocity time-series data                |
//! | GET    | `/api/approaches` | Paginated approach records with filters  |
//! | GET    | `/api/etl-runs`   | Recent ETL job execution history         |
//! | GET    | `/metrics`        | Prometheus metrics endpoint              |
//!
//! ## Example
//!
//! ```rust,no_run
//! use rustroid_sentinel::api::routes::api_router;
//! use rustroid_sentinel::server::AppState;
//! use axum::Router;
//!
//! // Create the API router with application state
//! fn create_app(state: AppState) -> Router {
//!     Router::new()
//!         .merge(api_router())
//!         .with_state(state)
//! }
//! ```

pub mod client;
pub mod error;
pub mod handlers;
pub mod routes;
pub mod templates;
pub mod types;
