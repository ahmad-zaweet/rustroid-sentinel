//! # Asteroid Model
//!
//! This module defines the [`Asteroid`] struct, which represents a near-Earth object
//! as stored and processed within the sentinel system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The UUID namespace used for generating deterministic asteroid IDs.
/// All asteroid UUIDs are derived from this namespace + the `neo_reference_id`.
pub const ASTEROID_UUID_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

/// A domain model representing near-Earth asteroid, decoupled from the NASA API DTO.
///
/// This is the internal representation used for persistence and business logic.
/// Asteroid identity is determined by `neo_reference_id`, which is NASA's unique identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asteroid {
    /// Deterministic UUID v5, generated from `neo_reference_id`.
    pub id: Uuid,
    /// NASA's unique identifier for this asteroid (natural key).
    pub neo_reference_id: String,
    /// Human-readable name or designation.
    pub name: String,
    /// The absolute visual magnitude (H), a measure of intrinsic brightness.
    pub absolute_magnitude: f64,
    /// Minimum estimated diameter in kilometers.
    pub estimated_diameter_min_km: f64,
    /// Maximum estimated diameter in kilometers.
    pub estimated_diameter_max_km: f64,
    /// Whether NASA designates this as a Potentially Hazardous Asteroid.
    pub is_potentially_hazardous: bool,
    /// Whether this asteroid is tracked by NASA's Sentry impact monitoring system.
    pub is_sentry_object: bool,
    /// Link to NASA's JPL Small-Body Database page.
    pub nasa_jpl_url: String,
    /// Timestamp when this record was first created.
    pub created_at: DateTime<Utc>,
    /// Timestamp when this record was last updated.
    pub updated_at: DateTime<Utc>,
}

impl Asteroid {
    /// Generates a deterministic UUID v5 for an asteroid from its `neo_reference_id`.
    ///
    /// This ensures the same asteroid always receives the same UUID, regardless
    /// of when or how many times it is processed.
    #[inline]
    pub fn generate_id(neo_reference_id: &str) -> Uuid {
        Uuid::new_v5(&ASTEROID_UUID_NAMESPACE, neo_reference_id.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_asteroid_generate_id() {
        let id1 = Asteroid::generate_id("20240101");
        let id2 = Asteroid::generate_id("20240101");
        let id3 = Asteroid::generate_id("20240102");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_asteroid_creation() {
        let asteroid = Asteroid {
            id: uuid::Uuid::new_v4(),
            neo_reference_id: "20240101".to_string(),
            name: "Test".to_string(),
            absolute_magnitude: 22.0,
            estimated_diameter_min_km: 0.5,
            estimated_diameter_max_km: 1.0,
            is_potentially_hazardous: false,
            is_sentry_object: false,
            nasa_jpl_url: "https://example.com".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(asteroid.neo_reference_id, "20240101");
        assert_eq!(asteroid.name, "Test");
        assert!(!asteroid.is_potentially_hazardous);
    }

    #[test]
    fn test_asteroid_clone() {
        let asteroid1 = Asteroid {
            id: uuid::Uuid::new_v4(),
            neo_reference_id: "20240101".to_string(),
            name: "Clone Test".to_string(),
            absolute_magnitude: 22.0,
            estimated_diameter_min_km: 0.5,
            estimated_diameter_max_km: 1.0,
            is_potentially_hazardous: true,
            is_sentry_object: false,
            nasa_jpl_url: "https://example.com".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let asteroid2 = asteroid1.clone();

        assert_eq!(asteroid1.neo_reference_id, asteroid2.neo_reference_id);
        assert_eq!(
            asteroid1.is_potentially_hazardous,
            asteroid2.is_potentially_hazardous
        );
    }
}
