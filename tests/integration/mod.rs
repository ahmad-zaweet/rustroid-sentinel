//! # Integration Tests
//!
//! Integration tests for Rustroid Sentinel.
//! These tests verify interactions between modules using real (but isolated) dependencies.
//!
//! ## Running Integration Tests
//!
//! ```bash
//! # Run all integration tests
//! cargo test --test integration
//!
//! # Run specific module
//! cargo test --test integration api
//! cargo test --test integration database
//!
//! # Run with output
//! cargo test --test integration -- --nocapture
//! ```
//!
//! ## Test Isolation
//!
//! Each test uses its own PostgreSQL container via testcontainers,
//! ensuring complete isolation between tests.

mod api;
mod database;
