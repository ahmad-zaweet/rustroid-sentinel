CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Powers a name-substring search filter on the catalog listing
-- (GET /asteroids), without a leading-wildcard full table scan.
CREATE INDEX IF NOT EXISTS idx_asteroids_name_trgm ON asteroids USING gin (name gin_trgm_ops);
