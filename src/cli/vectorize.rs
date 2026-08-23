//! The `vectorize` CLI subcommand.
//!
//! Computes the 16-dim pgvector embedding for every asteroid and upserts it
//! into `asteroid_embeddings`, powering `GET /api/asteroids/{id}/similar`.
//! Unlike `orbits`/`sentry`, this is pure computation over already-stored
//! data — no external API call, so every asteroid is recomputed every run
//! rather than tracked for staleness.

use anyhow::Result;
use clap::Args;
use tracing::info;

use crate::database::DatabasePool;
use crate::database::embeddings::EmbeddingRepository;
use crate::settings::RustroidSentinelConfig;
use crate::transform::embedding::normalize_features;

/// Arguments for the `vectorize` CLI subcommand.
#[derive(Args, Debug, Default)]
pub struct VectorizeArgs {}

/// Executes the vectorize command.
///
/// # Errors
///
/// Returns an error if the database connection, migrations, or the
/// feature-fetch/upsert queries fail.
pub async fn execute(_args: VectorizeArgs, settings: RustroidSentinelConfig) -> Result<()> {
    let db_pool = DatabasePool::new(&settings.database).await?;
    db_pool.run_migrations().await?;

    let feature_rows = EmbeddingRepository::fetch_feature_rows(db_pool.pool()).await?;
    info!(
        candidates = feature_rows.len(),
        "Computing asteroid embeddings"
    );

    if feature_rows.is_empty() {
        println!("No asteroids to vectorize.");
        return Ok(());
    }

    let embeddings: Vec<_> = feature_rows
        .into_iter()
        .map(|(id, features)| (id, normalize_features(&features)))
        .collect();
    let processed = embeddings.len();

    let updated = EmbeddingRepository::upsert_embeddings(db_pool.pool(), &embeddings).await?;

    info!(processed, updated, "Vectorize complete");
    println!("Processed {processed} asteroids: {updated} embeddings upserted");

    Ok(())
}
