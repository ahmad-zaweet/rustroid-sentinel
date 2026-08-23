ALTER TABLE asteroids
    ADD COLUMN IF NOT EXISTS torino_scale SMALLINT,
    ADD COLUMN IF NOT EXISTS palermo_scale DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS sentry_checked_at TIMESTAMPTZ;

-- Bounds the `sentry` CLI command's candidate-selection query
-- (is_sentry_object asteroids due for a re-check).
CREATE INDEX IF NOT EXISTS idx_asteroids_sentry_checked_at ON asteroids(sentry_checked_at);
