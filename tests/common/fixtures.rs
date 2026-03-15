//! # Test Fixtures
//!
//! Provides test data fixtures for NASA API responses,
//! asteroids, approaches, and other test data.

use chrono::{NaiveDate, Utc};
use rustroid_sentinel::models::{Approach, Asteroid};
use std::fs;
use uuid::Uuid;

/// NASA API fixture names.
#[derive(Debug, Clone, Copy)]
pub enum NasaFixtureName {
    /// Valid feed response with multiple asteroids
    FeedValid,
    /// Feed response with empty neo list
    FeedEmpty,
    /// Feed response with error status
    FeedError,
    /// Single asteroid lookup response
    AsteroidLookup,
}

impl NasaFixtureName {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FeedValid => "feed_valid.json",
            Self::FeedEmpty => "feed_empty.json",
            Self::FeedError => "feed_error.json",
            Self::AsteroidLookup => "asteroid_lookup.json",
        }
    }
}

/// Loads a NASA API fixture file.
///
/// # Panics
///
/// Panics if the fixture file is not found or invalid UTF-8.
pub fn load_nasa_fixture(name: NasaFixtureName) -> String {
    let path = format!("tests/fixtures/nasa_responses/{}", name.as_str());
    fs::read_to_string(&path).unwrap_or_else(|_| panic!("Fixture not found: {}", path))
}

/// Creates a test asteroid with default values.
pub fn create_test_asteroid() -> Asteroid {
    Asteroid {
        id: Uuid::new_v4(),
        neo_reference_id: "20240101".to_string(),
        name: "Test Asteroid".to_string(),
        absolute_magnitude: 22.5,
        estimated_diameter_min_km: 0.1,
        estimated_diameter_max_km: 0.5,
        is_potentially_hazardous: false,
        is_sentry_object: false,
        jpl_url: "https://ssd.jpl.nasa.gov/tools/sbdb_lookup.html#/?sstr=20240101".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

/// Creates a test approach with default values.
pub fn create_test_approach(asteroid_id: Uuid) -> Approach {
    Approach {
        id: Uuid::new_v4(),
        asteroid_id,
        close_approach_date: NaiveDate::from_ymd_opt(2024, 6, 15).unwrap(),
        velocity_km_per_h: 72000.0,
        miss_distance_km: 500000.0,
        orbiting_body: "Earth".to_string(),
        hazard_classification: None,
        created_at: Utc::now(),
    }
}

/// Creates multiple test asteroids.
pub fn create_test_asteroids(count: usize) -> Vec<Asteroid> {
    (0..count).map(|_| create_test_asteroid()).collect()
}

/// Creates test approaches for an asteroid.
pub fn create_test_approaches(asteroid_id: Uuid, count: usize) -> Vec<Approach> {
    (0..count)
        .map(|i| {
            let mut approach = create_test_approach(asteroid_id);
            approach.close_approach_date = NaiveDate::from_ymd_opt(2024, 6, i as u32 + 1).unwrap();
            approach
        })
        .collect()
}
