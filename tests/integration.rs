//! Integration test binary entry point.
//!
//! Cargo only compiles files placed directly under `tests/` as test binaries;
//! `tests/integration/` and `tests/common/` are otherwise inert directories.
//! This file is that entry point, wiring both in as submodules of the
//! `integration` test crate.

mod common;

#[path = "integration/mod.rs"]
mod integration;
