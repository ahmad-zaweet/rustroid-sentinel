ALTER TABLE asteroids
    ADD COLUMN IF NOT EXISTS estimated_diameter_avg_km DOUBLE PRECISION
    GENERATED ALWAYS AS ((estimated_diameter_min_km + estimated_diameter_max_km) / 2.0) STORED;
