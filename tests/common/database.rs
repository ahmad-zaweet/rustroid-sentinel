//! # Database Test Utilities
//!
//! Provides utilities for setting up isolated test databases
//! using Docker containers via bollard (secure alternative to testcontainers).

use bollard::{
    Docker,
    container::{Config, CreateContainerOptions, StartContainerOptions},
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::collections::HashMap;
use std::time::Duration;

/// Test database wrapper that manages lifecycle.
pub struct TestDatabase {
    container_id: String,
    docker: Docker,
    pool: PgPool,
    connection_string: String,
}

impl TestDatabase {
    /// Creates a new test database instance.
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let docker = Docker::connect_with_local_defaults()?;

        // Generate unique container name
        let container_name = format!("test-db-{}", uuid::Uuid::new_v4());

        // Create container config
        let mut env = HashMap::new();
        env.insert("POSTGRES_DB".to_string(), "test_db".to_string());
        env.insert("POSTGRES_USER".to_string(), "test_user".to_string());
        env.insert("POSTGRES_PASSWORD".to_string(), "test_pass".to_string());

        let config = Config {
            image: Some("postgres:15-alpine"),
            env: Some(
                env.into_iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<String>>(),
            ),
            ..Default::default()
        };

        let container = docker
            .create_container(
                Some(CreateContainerOptions {
                    name: &container_name,
                    platform: None,
                }),
                config,
            )
            .await?;

        // Start container (bollard 0.18 uses empty options struct)
        docker
            .start_container(&container.id, None::<StartContainerOptions<()>>)
            .await?;

        // Wait for postgres to start
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Get container port
        let container_info = docker.inspect_container(&container.id, None).await?;
        let host_port = container_info
            .network_settings
            .and_then(|ns| ns.ports)
            .and_then(|ports| ports.get("5432/tcp"))
            .and_then(|bindings| bindings.as_ref())
            .and_then(|bindings| bindings.first())
            .and_then(|b| b.host_port.as_ref())
            .ok_or("Failed to get container port")?;

        let connection_string = format!(
            "postgres://test_user:test_pass@localhost:{}/test_db",
            host_port
        );

        // Create connection pool
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .connect(&connection_string)
            .await?;

        Ok(Self {
            container_id: container.id,
            docker,
            pool,
            connection_string,
        })
    }

    /// Returns the database connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Returns the connection string.
    pub fn connection_string(&self) -> &str {
        &self.connection_string
    }

    /// Runs database migrations.
    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        let migration_001 = include_str!("../../migrations/001_create_tables.sql");
        let migration_002 = include_str!("../../migrations/002_create_alerts_table.sql");

        sqlx::raw_sql(migration_001).execute(&self.pool).await?;
        sqlx::raw_sql(migration_002).execute(&self.pool).await?;

        Ok(())
    }

    /// Clears all data from tables (for test isolation).
    pub async fn clear_data(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "TRUNCATE TABLE approaches, asteroids, alerts, etl_events RESTART IDENTITY CASCADE",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        // Clean up container asynchronously
        let docker = self.docker.clone();
        let container_id = self.container_id.clone();

        tokio::spawn(async move {
            let _ = docker.stop_container(&container_id, None).await;
            let _ = docker.remove_container(&container_id, None).await;
        });
    }
}

/// Sets up a test database and runs migrations.
pub async fn setup_test_database() -> Result<TestDatabase, Box<dyn std::error::Error>> {
    let db = TestDatabase::new().await?;
    db.run_migrations().await?;
    Ok(db)
}
