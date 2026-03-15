//! # NASA NeoWs API Response Types
//!
//! This module defines the data structures for deserializing responses from the
//! NASA Near Earth Object Web Service (NeoWs) API. These types represent the
//! raw API response before transformation into domain models.
//!
//! ## API Reference
//!
//! - [NASA NeoWs API Documentation](https://api.nasa.gov/neo/)
//! - [NeoWs Feed Endpoint](https://api.nasa.gov/neo/rest/v1/feed)

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the root of the NASA NeoWs feed response.
///
/// Contains metadata about the response and a map of objects organized by date.
/// This is the entry point for deserializing the `/feed` endpoint response.
///
/// # Fields
///
/// * `links` - Pagination navigation links
/// * `element_count` - Total count of NEOs across all dates in the request range
/// * `near_earth_objects` - Map of dates to vectors of NEOs (key format: YYYY-MM-DD)
#[derive(Debug, Deserialize, Serialize)]
pub struct NeoFeed {
    /// Navigation links for pagination (next, previous, self).
    pub links: Links,
    /// Total count of near-Earth objects across all dates in the request range.
    pub element_count: u32,
    /// A map where keys are dates (YYYY-MM-DD) and values are vectors of near-Earth objects.
    pub near_earth_objects: HashMap<String, Vec<NearEarthObject>>,
}

/// Represents the navigation links in the API response.
///
/// Provides pagination URLs for navigating through large result sets.
#[derive(Debug, Deserialize, Serialize)]
pub struct Links {
    /// Link to the next page of results (empty if on the last page).
    pub next: Option<String>,
    /// Link to the previous page of results (empty if on the first page).
    pub previous: Option<String>,
    /// Link to the current request (self-referential).
    #[serde(rename = "self")]
    pub self_link: String,
}

/// Represents a single Near-Earth Object (NEO) from the NASA API.
///
/// This struct contains all metadata and observation data for a single asteroid,
/// including physical properties, orbital data, and close approach events.
#[derive(Debug, Deserialize, Serialize)]
pub struct NearEarthObject {
    /// Self-referential links for this specific object.
    pub links: NeoLinks,
    /// Internal NASA ID for this object (numeric string).
    pub id: String,
    /// The unique NASA reference ID, also known as SPK-ID. Used as the natural key.
    pub neo_reference_id: String,
    /// Human-readable name or designation (e.g., "2015 XA142").
    pub name: String,
    /// URL to the JPL Small-Body Database page for detailed orbital elements.
    pub nasa_jpl_url: String,
    /// The absolute visual magnitude (H), a measure of intrinsic brightness.
    /// Lower values indicate larger/brighter objects.
    pub absolute_magnitude_h: f64,
    /// Estimated diameter range in multiple units (km, m, mi, ft).
    pub estimated_diameter: EstimatedDiameter,
    /// Whether this object is classified as a Potentially Hazardous Asteroid (PHA).
    pub is_potentially_hazardous_asteroid: bool,
    /// List of close approach events for this object within the requested date range.
    pub close_approach_data: Vec<CloseApproachData>,
    /// Whether this object is monitored by NASA's Sentry impact monitoring system.
    pub is_sentry_object: bool,
    /// Optional Sentry-specific data as a JSON string (if applicable).
    pub sentry_data: Option<String>,
}

/// Represents the 'self' link for a NEO.
#[derive(Debug, Deserialize, Serialize)]
pub struct NeoLinks {
    /// Self-referential API URL for this specific NEO.
    #[serde(rename = "self")]
    pub self_link: String,
}

/// Holds estimated diameter information in various units.
///
/// Contains min/max diameter estimates in kilometers, meters, miles, and feet.
#[derive(Debug, Deserialize, Serialize)]
pub struct EstimatedDiameter {
    /// Diameter range in kilometers.
    pub kilometers: DiameterRange,
    /// Diameter range in meters.
    pub meters: DiameterRange,
    /// Diameter range in miles.
    pub miles: DiameterRange,
    /// Diameter range in feet.
    pub feet: DiameterRange,
}

/// Represents a min/max range for diameter estimates.
///
/// The estimates are derived from the absolute magnitude and assumed albedo.
#[derive(Debug, Deserialize, Serialize)]
pub struct DiameterRange {
    /// Minimum estimated diameter in the specified unit.
    pub estimated_diameter_min: f64,
    /// Maximum estimated diameter in the specified unit.
    pub estimated_diameter_max: f64,
}

/// Contains data about a NEO's close approach to an orbiting body.
///
/// Each record represents a single approach event with timing, velocity,
/// and miss distance information.
#[derive(Debug, Deserialize, Serialize)]
pub struct CloseApproachData {
    /// The date of closest approach (format: YYYY-MM-DD).
    pub close_approach_date: NaiveDate,
    /// A human-readable full timestamp of the approach (format: YYYY-MM-dd HH:mm).
    pub close_approach_date_full: String,
    /// The epoch timestamp in milliseconds since 1970-01-01T00:00:00Z.
    pub epoch_date_close_approach: i64,
    /// Relative velocity at the time of approach in multiple units.
    pub relative_velocity: RelativeVelocity,
    /// The distance by which the object misses the orbiting body in multiple units.
    pub miss_distance: MissDistance,
    /// The name of the central body being orbited (e.g., "Earth").
    pub orbiting_body: String,
}

/// Holds relative velocity data in various units.
///
/// All values are deserialized from strings (NASA API format) to f64.
#[derive(Debug, Deserialize, Serialize)]
pub struct RelativeVelocity {
    /// Velocity in kilometers per second.
    #[serde(deserialize_with = "from_string_to_f64")]
    pub kilometers_per_second: f64,
    /// Velocity in kilometers per hour.
    #[serde(deserialize_with = "from_string_to_f64")]
    pub kilometers_per_hour: f64,
    /// Velocity in miles per hour.
    #[serde(deserialize_with = "from_string_to_f64")]
    pub miles_per_hour: f64,
}

/// Holds miss distance data in various units.
///
/// All values are deserialized from strings (NASA API format) to f64.
#[derive(Debug, Deserialize, Serialize)]
pub struct MissDistance {
    /// Miss distance in astronomical units (AU).
    #[serde(deserialize_with = "from_string_to_f64")]
    pub astronomical: f64,
    /// Miss distance in lunar distances (LD).
    #[serde(deserialize_with = "from_string_to_f64")]
    pub lunar: f64,
    /// Miss distance in kilometers.
    #[serde(deserialize_with = "from_string_to_f64")]
    pub kilometers: f64,
    /// Miss distance in miles.
    #[serde(deserialize_with = "from_string_to_f64")]
    pub miles: f64,
}

/// A custom deserializer to convert either a string or a number to an f64.
///
/// NASA's API returns these fields as strings, but after our extract step
/// serializes the data, serde may write them as numbers. This handles both.
///
/// # Examples
///
/// ```rust,ignore
/// // Can deserialize from string: "25.5"
/// // Can deserialize from number: 25.5
/// ```
fn from_string_to_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => s.parse::<f64>().map_err(serde::de::Error::custom),
        serde_json::Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| serde::de::Error::custom("Expected a valid number")),
        _ => Err(serde::de::Error::custom(
            "Expected a string or number for f64 field",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct TestStruct {
        #[serde(deserialize_with = "from_string_to_f64")]
        value: f64,
    }

    #[test]
    fn test_from_string_to_f64_with_string() {
        let json = r#"{"value": "123.45"}"#;
        let result: TestStruct = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, 123.45);
    }

    #[test]
    fn test_from_string_to_f64_with_number() {
        let json = r#"{"value": 123.45}"#;
        let result: TestStruct = serde_json::from_str(json).unwrap();
        assert_eq!(result.value, 123.45);
    }

    #[test]
    fn test_from_string_to_f64_with_invalid_string() {
        let json = r#"{"value": "not-a-number"}"#;
        let result: Result<TestStruct, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_string_to_f64_with_invalid_type() {
        let json = r#"{"value": [1, 2]}"#;
        let result: Result<TestStruct, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
