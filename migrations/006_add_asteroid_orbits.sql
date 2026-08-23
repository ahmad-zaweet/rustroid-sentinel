CREATE TABLE IF NOT EXISTS asteroid_orbits (
    asteroid_id UUID PRIMARY KEY REFERENCES asteroids(id) ON DELETE CASCADE,
    eccentricity DOUBLE PRECISION,
    semi_major_axis_au DOUBLE PRECISION,
    inclination_deg DOUBLE PRECISION,
    ascending_node_deg DOUBLE PRECISION,
    perihelion_arg_deg DOUBLE PRECISION,
    mean_anomaly_deg DOUBLE PRECISION,
    orbital_period_days DOUBLE PRECISION,
    orbit_class TEXT,
    spectral_class TEXT,
    albedo DOUBLE PRECISION,
    orbit_checked_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Bounds the `orbits` CLI command's candidate-selection query (asteroids
-- with a missing or stale orbit row).
CREATE INDEX IF NOT EXISTS idx_asteroid_orbits_checked_at ON asteroid_orbits(orbit_checked_at);
