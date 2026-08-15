//! # Persistence Layer
//!
//! This module provides the infrastructure for interacting with the PostgreSQL
//! database. It includes a managed connection pool ([`DatabasePool`]),
//! safe migration execution, and repository traits for domain entity persistence.

pub mod dashboard;
pub mod error;
pub mod repository;
pub mod retention;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tracing::{error, info};

use crate::settings::DatabaseConfig;

/// A wrapper around `sqlx::PgPool` providing managed database connectivity.
///
/// This struct handles connection pool creation and configuration based
/// on the application's [`DatabaseConfig`]. It is safely shareable across
/// multiple threads and tasks as it internally uses an `Arc`.
#[derive(Debug, Clone)]
pub struct DatabasePool {
    pool: PgPool,
}

impl DatabasePool {
    /// Creates a new `DatabasePool` from the given configuration.
    ///
    /// Establishes an asynchronous connection pool with the configured min/max
    /// connections and acquisition timeout settings.
    ///
    /// # Arguments
    ///
    /// * `config` - A reference to the [`DatabaseConfig`] struct containing the connection string and pool settings.
    ///
    /// # Errors
    ///
    /// Returns a [`sqlx::Error`] if the data source URL is invalid, if the pool
    /// cannot be established, or if the connection attempt times out.
    pub async fn new(config: &DatabaseConfig) -> Result<Self, sqlx::Error> {
        info!("Initializing database connection pool...");

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connect_timeout_seconds))
            .connect(&config.url)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to create database pool");
                e
            })?;

        info!(
            max_connections = config.max_connections,
            min_connections = config.min_connections,
            "Database connection pool established"
        );

        Ok(Self { pool })
    }

    /// Returns a reference to the underlying [`PgPool`].
    ///
    /// This can be used to perform ad-hoc queries or interact directly with `sqlx`.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Executes the embedded SQL migration files to initialize or update the schema.
    ///
    /// This method sequentially runs:
    /// 1. `migrations/001_create_tables.sql`
    /// 2. `migrations/002_create_alerts_table.sql`
    /// 3. `migrations/003_add_diameter_avg_column.sql`
    /// 4. `migrations/004_add_etl_events_started_at_index.sql`
    ///
    /// # Errors
    ///
    /// Returns a [`sqlx::Error`] if any migration fails to execute or if the
    /// database connection is lost during the process.
    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        info!("Running database migrations...");

        let migration_sql = include_str!("../../migrations/001_create_tables.sql");
        sqlx::raw_sql(migration_sql).execute(&self.pool).await?;

        let migration_sql_002 = include_str!("../../migrations/002_create_alerts_table.sql");
        sqlx::raw_sql(migration_sql_002).execute(&self.pool).await?;

        let migration_sql_003 = include_str!("../../migrations/003_add_diameter_avg_column.sql");
        sqlx::raw_sql(migration_sql_003).execute(&self.pool).await?;

        let migration_sql_004 =
            include_str!("../../migrations/004_add_etl_events_started_at_index.sql");
        sqlx::raw_sql(migration_sql_004).execute(&self.pool).await?;

        info!("Database migrations completed successfully");
        Ok(())
    }
}
