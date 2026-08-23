CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS asteroid_embeddings (
    asteroid_id UUID PRIMARY KEY REFERENCES asteroids(id) ON DELETE CASCADE,
    embedding vector(16) NOT NULL,
    computed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- HNSW index, rebuilt/refreshed only by the `vectorize` CLI job, never on
-- request (M5 constraint: no index writes triggered by user traffic).
CREATE INDEX IF NOT EXISTS idx_asteroid_embeddings_hnsw
    ON asteroid_embeddings USING hnsw (embedding vector_l2_ops);
