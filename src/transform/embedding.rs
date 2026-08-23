//! 16-dimension feature embedding for pgvector similarity search (M5).
//!
//! Pure computation, no DB dependency — mirrors [`super::classify_hazard`].
//! Each feature is min-max scaled into `[0.0, 1.0]` against a fixed domain
//! range (no full-table stats scan needed, and the ranges stay stable across
//! reindex runs so distances remain comparable over time).
//!
//! Missing values fall into two distinct categories, each imputed
//! differently:
//! - Torino/Palermo scale absence means "not a tracked virtual impactor" —
//!   a real, benign value, not missing data — so it's imputed at the benign
//!   end of the range (0 / -10).
//! - Orbital-element and approach-derived fields are genuinely sparse (SBDB
//!   coverage is partial, some asteroids have no recorded approaches) — so
//!   they're imputed at the midpoint (0.5), which biases them toward
//!   "neutral" rather than toward either extreme.

use pgvector::Vector;

/// Raw inputs feeding the 16-dim embedding. All orbital/approach fields are
/// `Option` because SBDB coverage and approach history are both sparse.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AsteroidFeatures {
    /// Absolute magnitude (H), always present.
    pub absolute_magnitude: f64,
    /// Average estimated diameter in kilometers, always present.
    pub estimated_diameter_avg_km: f64,
    /// Whether NASA designates this as a Potentially Hazardous Asteroid.
    pub is_potentially_hazardous: bool,
    /// Whether this asteroid is currently on JPL's Sentry Risk List.
    pub is_sentry_object: bool,
    /// JPL Sentry Torino Scale (0-10); `None` if not sentry-flagged.
    pub torino_scale: Option<i16>,
    /// JPL Sentry cumulative Palermo Scale; `None` if not sentry-flagged.
    pub palermo_scale: Option<f64>,
    /// Orbital eccentricity, from JPL's SBDB (sparse).
    pub eccentricity: Option<f64>,
    /// Semi-major axis in AU, from JPL's SBDB (sparse).
    pub semi_major_axis_au: Option<f64>,
    /// Inclination to the ecliptic in degrees, from JPL's SBDB (sparse).
    pub inclination_deg: Option<f64>,
    /// Longitude of the ascending node in degrees, from JPL's SBDB (sparse).
    pub ascending_node_deg: Option<f64>,
    /// Argument of perihelion in degrees, from JPL's SBDB (sparse).
    pub perihelion_arg_deg: Option<f64>,
    /// Mean anomaly in degrees, from JPL's SBDB (sparse).
    pub mean_anomaly_deg: Option<f64>,
    /// Orbital period in days, from JPL's SBDB (sparse).
    pub orbital_period_days: Option<f64>,
    /// Geometric albedo, from JPL's SBDB (sparse).
    pub albedo: Option<f64>,
    /// Velocity at the closest recorded approach (`MIN(miss_distance_km)`).
    pub velocity_km_per_s: Option<f64>,
    /// Miss distance, in AU, at the closest recorded approach.
    pub miss_distance_astronomical: Option<f64>,
}

/// Number of dimensions in the embedding. Must match the `vector(16)` column
/// in `migrations/008_add_asteroid_embeddings.sql`.
pub const EMBEDDING_DIMS: usize = 16;

/// Midpoint imputation for genuinely-missing sparse fields — neutral rather
/// than biasing toward either extreme of the range.
const MIDPOINT: f32 = 0.5;

/// Min-max scales `value` into `[0.0, 1.0]` against `[min, max]`, clamping
/// out-of-range inputs rather than letting them distort the vector space.
fn scale(value: f64, min: f64, max: f64) -> f32 {
    (((value - min) / (max - min)) as f32).clamp(0.0, 1.0)
}

/// Scales an optional field, imputing `MIDPOINT` when absent (genuinely
/// missing SBDB/approach data — see module docs).
fn scale_opt(value: Option<f64>, min: f64, max: f64) -> f32 {
    value.map_or(MIDPOINT, |v| scale(v, min, max))
}

/// Builds the 16-dim feature embedding for one asteroid.
///
/// Field order is fixed and documented in `docs/NEON_SERVERLESS_PLAN.md`
/// (M5) — changing it invalidates every previously-computed embedding, since
/// distances are only meaningful between vectors built with the same layout.
#[must_use]
pub fn normalize_features(features: &AsteroidFeatures) -> Vector {
    let torino = features.torino_scale.map(f64::from);
    let values = [
        scale(features.absolute_magnitude, 9.0, 33.0),
        scale(features.estimated_diameter_avg_km, 0.0, 40.0),
        f32::from(features.is_potentially_hazardous),
        f32::from(features.is_sentry_object),
        // Benign-endpoint imputation: absence means "not sentry-flagged",
        // not "unknown" — see module docs.
        torino.map_or(0.0, |t| scale(t, 0.0, 10.0)),
        features.palermo_scale.map_or(0.0, |p| scale(p, -10.0, 2.0)),
        scale_opt(features.eccentricity, 0.0, 1.0),
        scale_opt(features.semi_major_axis_au, 0.0, 5.0),
        scale_opt(features.inclination_deg, 0.0, 180.0),
        scale_opt(features.ascending_node_deg, 0.0, 360.0),
        scale_opt(features.perihelion_arg_deg, 0.0, 360.0),
        scale_opt(features.mean_anomaly_deg, 0.0, 360.0),
        scale_opt(features.orbital_period_days, 0.0, 3000.0),
        scale_opt(features.albedo, 0.0, 1.0),
        scale_opt(features.velocity_km_per_s, 0.0, 50.0),
        scale_opt(features.miss_distance_astronomical, 0.0, 0.5),
    ];

    Vector::from(values.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_of(f: &AsteroidFeatures) -> Vec<f32> {
        normalize_features(f).to_vec()
    }

    #[test]
    fn dimension_count_matches_column_width() {
        let features = AsteroidFeatures::default();
        assert_eq!(vec_of(&features).len(), EMBEDDING_DIMS);
    }

    #[test]
    fn scales_within_range_linearly() {
        let features = AsteroidFeatures {
            absolute_magnitude: 21.0,        // midpoint of [9, 33]
            estimated_diameter_avg_km: 20.0, // midpoint of [0, 40]
            ..Default::default()
        };
        let v = vec_of(&features);
        assert!((v[0] - 0.5).abs() < 1e-6);
        assert!((v[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn clamps_out_of_range_values() {
        let features = AsteroidFeatures {
            absolute_magnitude: 100.0,       // way above max
            estimated_diameter_avg_km: -5.0, // below min
            ..Default::default()
        };
        let v = vec_of(&features);
        assert_eq!(v[0], 1.0);
        assert_eq!(v[1], 0.0);
    }

    #[test]
    fn hazard_scale_absence_imputes_to_benign_endpoint() {
        let features = AsteroidFeatures {
            torino_scale: None,
            palermo_scale: None,
            ..Default::default()
        };
        let v = vec_of(&features);
        assert_eq!(v[4], 0.0, "missing torino should impute to 0, not midpoint");
        assert_eq!(
            v[5], 0.0,
            "missing palermo should impute to scaled -10, not midpoint"
        );
    }

    #[test]
    fn hazard_scale_present_scales_normally() {
        let features = AsteroidFeatures {
            torino_scale: Some(10),
            palermo_scale: Some(2.0),
            ..Default::default()
        };
        let v = vec_of(&features);
        assert_eq!(v[4], 1.0);
        assert_eq!(v[5], 1.0);
    }

    #[test]
    fn sparse_orbital_fields_impute_to_midpoint() {
        let features = AsteroidFeatures::default();
        let v = vec_of(&features);
        for &x in &v[6..16] {
            assert!(
                (x - 0.5).abs() < 1e-6,
                "sparse field should impute to midpoint, got {x}"
            );
        }
    }

    #[test]
    fn boolean_flags_encode_as_zero_one() {
        let mut features = AsteroidFeatures::default();
        let v_false = vec_of(&features);
        assert_eq!(v_false[2], 0.0);
        assert_eq!(v_false[3], 0.0);

        features.is_potentially_hazardous = true;
        features.is_sentry_object = true;
        let v_true = vec_of(&features);
        assert_eq!(v_true[2], 1.0);
        assert_eq!(v_true[3], 1.0);
    }
}
