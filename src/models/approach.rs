//! # Close Approach Model
//!
//! This module defines the [`Approach`] struct, which represents a specific instance
//! of an asteroid passing near Earth.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::HazardClassification;

/// The UUID namespace used for generating deterministic approach IDs.
/// All approach UUIDs are derived from this namespace + `neo_reference_id:epoch`.
pub const APPROACH_UUID_NAMESPACE: Uuid = Uuid::from_bytes([
    0x7c, 0xb8, 0xc9, 0x21, 0xae, 0xbe, 0x22, 0xe2, 0x91, 0xc5, 0x11, 0xd1, 0x5f, 0xe5, 0x41, 0xd9,
]);

/// A domain model representing a close approach event of an asteroid to Earth.
///
/// Each approach is uniquely identified by the combination of the asteroid
/// and the epoch timestamp of closest approach.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approach {
    /// Deterministic UUID v5, generated from `neo_reference_id` + `epoch_date_close_approach`.
    pub id: Uuid,
    /// Foreign key referencing the parent `Asteroid`.
    pub asteroid_id: Uuid,
    /// The date of closest approach.
    pub close_approach_date: NaiveDate,
    /// The epoch timestamp (milliseconds) of closest approach.
    pub epoch_date_close_approach: i64,
    /// Relative velocity in kilometers per second.
    pub velocity_km_per_s: f64,
    /// Relative velocity in kilometers per hour.
    pub velocity_km_per_h: f64,
    /// Miss distance in kilometers.
    pub miss_distance_km: f64,
    /// Miss distance in astronomical units.
    pub miss_distance_astronomical: f64,
    /// Miss distance in lunar distances.
    pub miss_distance_lunar: f64,
    /// The body being orbited during the approach (typically "Earth").
    pub orbiting_body: String,
    /// The computed hazard classification for this approach.
    pub hazard_classification: HazardClassification,
    /// Timestamp when this record was first created.
    pub created_at: DateTime<Utc>,
}

impl Approach {
    /// Generates a deterministic UUID v5 for an approach from the asteroid's
    /// `neo_reference_id` and the epoch timestamp.
    ///
    /// This ensures the same approach event always receives the same UUID.
    #[inline]
    pub fn generate_id(neo_reference_id: &str, epoch_date_close_approach: i64) -> Uuid {
        let name = format!("{}:{}", neo_reference_id, epoch_date_close_approach);
        Uuid::new_v5(&APPROACH_UUID_NAMESPACE, name.as_bytes())
    }
}
