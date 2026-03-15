CREATE TABLE IF NOT EXISTS asteroids (
    id UUID PRIMARY KEY,
    neo_reference_id TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    absolute_magnitude DOUBLE PRECISION NOT NULL,
    estimated_diameter_min_km DOUBLE PRECISION NOT NULL,
    estimated_diameter_max_km DOUBLE PRECISION NOT NULL,
    is_potentially_hazardous BOOLEAN NOT NULL DEFAULT FALSE,
    is_sentry_object BOOLEAN NOT NULL DEFAULT FALSE,
    nasa_jpl_url TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_asteroids_neo_reference_id ON asteroids(neo_reference_id);
CREATE INDEX IF NOT EXISTS idx_asteroids_is_potentially_hazardous ON asteroids(is_potentially_hazardous);

CREATE TABLE IF NOT EXISTS approaches (
    id UUID PRIMARY KEY,
    asteroid_id UUID NOT NULL REFERENCES asteroids(id) ON DELETE CASCADE,
    close_approach_date DATE NOT NULL,
    epoch_date_close_approach BIGINT NOT NULL,
    velocity_km_per_s DOUBLE PRECISION NOT NULL,
    velocity_km_per_h DOUBLE PRECISION NOT NULL,
    miss_distance_km DOUBLE PRECISION NOT NULL,
    miss_distance_astronomical DOUBLE PRECISION NOT NULL,
    miss_distance_lunar DOUBLE PRECISION NOT NULL,
    orbiting_body TEXT NOT NULL DEFAULT 'Earth',
    hazard_classification TEXT NOT NULL DEFAULT 'Low',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (asteroid_id, epoch_date_close_approach)
);

CREATE INDEX IF NOT EXISTS idx_approaches_asteroid_id ON approaches(asteroid_id);
CREATE INDEX IF NOT EXISTS idx_approaches_close_approach_date ON approaches(close_approach_date);
CREATE INDEX IF NOT EXISTS idx_approaches_hazard_classification ON approaches(hazard_classification);

CREATE TABLE IF NOT EXISTS etl_events (
    id UUID PRIMARY KEY,
    source_file TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'running',
    asteroids_processed INTEGER NOT NULL DEFAULT 0,
    approaches_processed INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    UNIQUE (source_file)
);
