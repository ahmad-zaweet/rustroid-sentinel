//! # Database Test Utilities
//!
//! Provides utilities for setting up isolated test databases
//! using Docker containers via bollard (secure alternative to testcontainers).

use anyhow::{Context, Result};
use bollard::{
    Docker,
    container::{Config, CreateContainerOptions, StartContainerOptions},
    models::HostConfig,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::collections::HashMap;
use std::time::Duration;

/// Test database wrapper that manages lifecycle.
pub struct TestDatabase {
    container_id: String,
    docker: Docker,
    pool: PgPool,
}

impl TestDatabase {
    /// Creates a new test database instance.
    pub async fn new() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;

        // Generate unique container name
        let container_name = format!("test-db-{}", uuid::Uuid::new_v4());

        // Create container config
        let mut env = HashMap::new();
        env.insert("POSTGRES_DB".to_string(), "test_db".to_string());
        env.insert("POSTGRES_USER".to_string(), "test_user".to_string());
        env.insert("POSTGRES_PASSWORD".to_string(), "test_pass".to_string());

        let env_pairs: Vec<String> = env
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        let env_refs: Vec<&str> = env_pairs.iter().map(String::as_str).collect();

        let config = Config {
            image: Some("postgres:17-alpine"),
            env: Some(env_refs),
            host_config: Some(HostConfig {
                publish_all_ports: Some(true),
                ..Default::default()
            }),
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

        // Start container
        docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await?;

        // Ports are published as soon as the container starts; no need to
        // wait for that.
        let container_info = docker.inspect_container(&container.id, None).await?;
        let host_port = container_info
            .network_settings
            .and_then(|ns| ns.ports)
            .and_then(|ports| ports.get("5432/tcp").cloned())
            .flatten()
            .and_then(|bindings| bindings.into_iter().next())
            .and_then(|binding| binding.host_port)
            .context("Failed to get container port")?;

        let connection_string = format!(
            "postgres://test_user:test_pass@localhost:{}/test_db",
            host_port
        );

        // Postgres restarts once internally after `initdb` on first boot, so
        // the port can accept (and then drop) connections before it's truly
        // ready. Retry rather than guessing a fixed sleep duration.
        let pool = {
            let mut last_err = None;
            let mut pool = None;
            for _ in 0..30 {
                match PgPoolOptions::new()
                    .max_connections(5)
                    .acquire_timeout(Duration::from_secs(5))
                    .connect(&connection_string)
                    .await
                {
                    Ok(p) => {
                        pool = Some(p);
                        break;
                    }
                    Err(e) => {
                        last_err = Some(e);
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
            pool.ok_or_else(|| {
                anyhow::anyhow!("Postgres container never became ready: {:?}", last_err)
            })?
        };

        Ok(Self {
            container_id: container.id,
            docker,
            pool,
        })
    }

    /// Returns the database connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Runs database migrations.
    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        let migration_001 = include_str!("../../migrations/001_create_tables.sql");
        let migration_002 = include_str!("../../migrations/002_create_alerts_table.sql");
        let migration_003 = include_str!("../../migrations/003_add_diameter_avg_column.sql");
        let migration_004 =
            include_str!("../../migrations/004_add_etl_events_started_at_index.sql");

        sqlx::raw_sql(migration_001).execute(&self.pool).await?;
        sqlx::raw_sql(migration_002).execute(&self.pool).await?;
        sqlx::raw_sql(migration_003).execute(&self.pool).await?;
        sqlx::raw_sql(migration_004).execute(&self.pool).await?;

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
