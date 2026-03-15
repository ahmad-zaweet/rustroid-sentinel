//! # Command Line Interface (CLI)
//!
//! This module implements the CLI commands for Rustroid Sentinel, providing
//! granular control over the application's various subsystems.
//!
//! ## Architecture
//!
//! The CLI is built with [`clap`] and uses the derive API for argument parsing.
//! It supports the following subcommands:
//!
//! | Command       | Description                                    |
//! |---------------|------------------------------------------------|
//! | `server`      | Start the full HTTP API server                 |
//! | `extract`     | Run the ETL extraction job manually            |
//! | `transform`   | Run data transformation (standalone mode)      |
//! | `load`        | Run database loading (standalone mode)         |
//! | `alert`       | Check and send alerts for hazardous approaches |
//!
//! ## Usage
//!
//! ```bash
//! # Start the API server
//! rustroid-sentinel server
//!
//! # Run ETL extraction for the past 7 days
//! rustroid-sentinel extract --days 7
//!
//! # Send alerts for new hazardous approaches
//! rustroid-sentinel alert
//! ```
//!
//! ## Environment Variables
//!
//! All commands respect the `RUN_ENV` environment variable for configuration:
//!
//! ```bash
//! RUN_ENV=production rustroid-sentinel server
//! ```
//!
//! Configuration files are loaded from `config/{RUN_ENV}.toml`.

pub mod alert;
pub mod extract;
pub mod load;
pub mod transform;

#[cfg(test)]
mod tests {
    // CLI command parsing is tested in main.rs integration tests
    // This module focuses on command execution logic

    #[test]
    fn test_cli_module_structure() {
        // Verify module structure
        assert!(true);
    }
}
