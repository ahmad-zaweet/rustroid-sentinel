//! # Transformation & Classification Logic
//!
//! This module handles the conversion of raw NASA NeoWs API responses into our
//! internal domain models. It also contains the core logic for assessing the
//! threat level of each asteroid approach.

use chrono::Utc;
use tracing::{debug, info, warn};

use crate::models::HazardClassification;
use crate::models::approach::Approach;
use crate::models::asteroid::Asteroid;
use crate::nasa::asteroid_neows::responses::NeoFeed;

/// Transforms a NASA NeoWs API feed response into domain models.
///
/// Each `NearEarthObject` in the feed is converted into an [`Asteroid`], and each
/// of its `CloseApproachData` entries becomes an [`Approach`] linked to that asteroid.
///
/// # Arguments
///
/// * `feed` - A reference to the deserialized NASA [`NeoFeed`] response.
///
/// # Returns
///
/// A vector of tuples, each containing an [`Asteroid`] and its associated `Vec<Approach>`.
pub fn transform_neo_feed(feed: &NeoFeed) -> Vec<(Asteroid, Vec<Approach>)> {
    let mut results = Vec::with_capacity(feed.element_count as usize);
    let now = Utc::now();

    for neos in feed.near_earth_objects.values() {
        for neo in neos {
            let asteroid_id = Asteroid::generate_id(&neo.neo_reference_id);

            let asteroid = Asteroid {
                id: asteroid_id,
                neo_reference_id: neo.neo_reference_id.clone(),
                name: neo.name.clone(),
                absolute_magnitude: neo.absolute_magnitude_h,
                estimated_diameter_min_km: neo.estimated_diameter.kilometers.estimated_diameter_min,
                estimated_diameter_max_km: neo.estimated_diameter.kilometers.estimated_diameter_max,
                is_potentially_hazardous: neo.is_potentially_hazardous_asteroid,
                is_sentry_object: neo.is_sentry_object,
                nasa_jpl_url: neo.nasa_jpl_url.clone(),
                created_at: now,
                updated_at: now,
            };

            let mut approaches = Vec::with_capacity(neo.close_approach_data.len());
            for cad in &neo.close_approach_data {
                let classification = classify_hazard(
                    neo.estimated_diameter.kilometers.estimated_diameter_max,
                    cad.relative_velocity.kilometers_per_second,
                    cad.miss_distance.astronomical,
                    neo.is_potentially_hazardous_asteroid,
                );

                let approach_id =
                    Approach::generate_id(&neo.neo_reference_id, cad.epoch_date_close_approach);

                approaches.push(Approach {
                    id: approach_id,
                    asteroid_id,
                    close_approach_date: cad.close_approach_date,
                    epoch_date_close_approach: cad.epoch_date_close_approach,
                    velocity_km_per_s: cad.relative_velocity.kilometers_per_second,
                    velocity_km_per_h: cad.relative_velocity.kilometers_per_hour,
                    miss_distance_km: cad.miss_distance.kilometers,
                    miss_distance_astronomical: cad.miss_distance.astronomical,
                    miss_distance_lunar: cad.miss_distance.lunar,
                    orbiting_body: cad.orbiting_body.clone(),
                    hazard_classification: classification,
                    created_at: now,
                });
            }

            debug!(
                neo_id = %neo.neo_reference_id,
                name = %neo.name,
                approaches = approaches.len(),
                "Transformed asteroid"
            );

            results.push((asteroid, approaches));
        }
    }

    let total_asteroids = results.len();
    let total_approaches: usize = results.iter().map(|(_, a)| a.len()).sum();
    info!(total_asteroids, total_approaches, "Transformation complete");

    results
}

/// Classifies the hazard level of a near-Earth approach based on physical and
/// orbital parameters.
///
/// # Classification Rules
///
/// The classification is determined using a hierarchical approach (first match wins):
///
/// 1. **Critical**: NASA PHA designation AND miss distance < 0.05 AU.
/// 2. **High**: Large asteroid (max diameter > 0.5 km) OR (fast velocity > 20 km/s AND close approach < 0.1 AU).
/// 3. **Medium**: Medium-sized asteroid (max diameter > 0.1 km) OR moderately fast velocity (> 10 km/s).
/// 4. **Low**: Any approach not meeting the above criteria.
///
/// # Arguments
///
/// * `diameter_max_km` - Maximum estimated diameter in kilometers.
/// * `velocity_km_s` - Relative velocity in kilometers per second.
/// * `miss_distance_au` - Miss distance in astronomical units.
/// * `is_pha` - Whether NASA designates this as a Potentially Hazardous Asteroid.
///
/// # Examples
///
/// ```rust
/// use rustroid_sentinel::transform::classify_hazard;
/// use rustroid_sentinel::models::HazardClassification;
///
/// // A PHA passing very close is Critical
/// let hazard = classify_hazard(0.3, 15.0, 0.04, true);
/// assert_eq!(hazard, HazardClassification::Critical);
///
/// // A large non-PHA is still High risk
/// let hazard = classify_hazard(0.6, 5.0, 0.5, false);
/// assert_eq!(hazard, HazardClassification::High);
///
/// // A small, slow, and distant object is Low risk
/// let hazard = classify_hazard(0.05, 5.0, 0.5, false);
/// assert_eq!(hazard, HazardClassification::Low);
/// ```
#[inline]
pub fn classify_hazard(
    diameter_max_km: f64,
    velocity_km_s: f64,
    miss_distance_au: f64,
    is_pha: bool,
) -> HazardClassification {
    // Critical: NASA PHA + very close approach
    if is_pha && miss_distance_au < 0.05 {
        warn!(
            diameter_max_km,
            velocity_km_s,
            miss_distance_au,
            "CRITICAL hazard classification: PHA with very close approach"
        );
        return HazardClassification::Critical;
    }

    // High: Large asteroid OR fast + close
    if diameter_max_km > 0.5 || (velocity_km_s > 20.0 && miss_distance_au < 0.1) {
        return HazardClassification::High;
    }

    // Medium: medium-sized or moderately fast
    if diameter_max_km > 0.1 || velocity_km_s > 10.0 {
        return HazardClassification::Medium;
    }

    // Low: everything else
    HazardClassification::Low
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_critical() {
        let result = classify_hazard(0.3, 15.0, 0.04, true);
        assert_eq!(result, HazardClassification::Critical);
    }

    #[test]
    fn test_classify_high_large_diameter() {
        let result = classify_hazard(0.6, 5.0, 0.5, false);
        assert_eq!(result, HazardClassification::High);
    }

    #[test]
    fn test_classify_high_fast_and_close() {
        let result = classify_hazard(0.05, 25.0, 0.08, false);
        assert_eq!(result, HazardClassification::High);
    }

    #[test]
    fn test_classify_medium_diameter() {
        let result = classify_hazard(0.15, 5.0, 0.5, false);
        assert_eq!(result, HazardClassification::Medium);
    }

    #[test]
    fn test_classify_medium_velocity() {
        let result = classify_hazard(0.05, 12.0, 0.5, false);
        assert_eq!(result, HazardClassification::Medium);
    }

    #[test]
    fn test_classify_low() {
        let result = classify_hazard(0.05, 5.0, 0.5, false);
        assert_eq!(result, HazardClassification::Low);
    }

    #[test]
    fn test_pha_not_close_enough_for_critical() {
        // PHA but not close enough → should NOT be Critical
        let result = classify_hazard(0.3, 15.0, 0.1, true);
        assert_eq!(result, HazardClassification::Medium);
    }

    #[test]
    fn test_deterministic_asteroid_uuid() {
        let id1 = Asteroid::generate_id("2496818");
        let id2 = Asteroid::generate_id("2496818");
        let id3 = Asteroid::generate_id("3826807");

        assert_eq!(id1, id2, "Same neo_reference_id should produce same UUID");
        assert_ne!(
            id1, id3,
            "Different neo_reference_id should produce different UUID"
        );
    }

    #[test]
    fn test_deterministic_approach_uuid() {
        let id1 = Approach::generate_id("2496818", 1764916440000);
        let id2 = Approach::generate_id("2496818", 1764916440000);
        let id3 = Approach::generate_id("2496818", 1764946560000);

        assert_eq!(id1, id2, "Same inputs should produce same UUID");
        assert_ne!(id1, id3, "Different epoch should produce different UUID");
    }

    #[test]
    fn test_transform_neo_feed() {
        use crate::nasa::asteroid_neows::responses::{
            CloseApproachData, DiameterRange, EstimatedDiameter, Links, MissDistance,
            NearEarthObject, NeoFeed, NeoLinks, RelativeVelocity,
        };
        use std::collections::HashMap;

        let today = Utc::now().date_naive();
        let neo = NearEarthObject {
            links: NeoLinks {
                self_link: "url".to_string(),
            },
            id: "123".to_string(),
            neo_reference_id: "REF123".to_string(),
            name: "Asteroid Zero".to_string(),
            nasa_jpl_url: "jpl".to_string(),
            absolute_magnitude_h: 20.0,
            estimated_diameter: EstimatedDiameter {
                kilometers: DiameterRange {
                    estimated_diameter_min: 1.0,
                    estimated_diameter_max: 2.0,
                },
                meters: DiameterRange {
                    estimated_diameter_min: 1000.0,
                    estimated_diameter_max: 2000.0,
                },
                miles: DiameterRange {
                    estimated_diameter_min: 0.6,
                    estimated_diameter_max: 1.2,
                },
                feet: DiameterRange {
                    estimated_diameter_min: 3000.0,
                    estimated_diameter_max: 6000.0,
                },
            },
            is_potentially_hazardous_asteroid: true,
            close_approach_data: vec![CloseApproachData {
                close_approach_date: today,
                close_approach_date_full: "today".to_string(),
                epoch_date_close_approach: 123456789,
                relative_velocity: RelativeVelocity {
                    kilometers_per_second: 15.0,
                    kilometers_per_hour: 54000.0,
                    miles_per_hour: 33000.0,
                },
                miss_distance: MissDistance {
                    astronomical: 0.04,
                    lunar: 15.0,
                    kilometers: 6_000_000.0,
                    miles: 3_700_000.0,
                },
                orbiting_body: "Earth".to_string(),
            }],
            is_sentry_object: false,
            sentry_data: None,
        };

        let mut object_map = HashMap::new();
        object_map.insert(today.to_string(), vec![neo]);

        let feed = NeoFeed {
            links: Links {
                next: None,
                previous: None,
                self_link: "".to_string(),
            },
            element_count: 1,
            near_earth_objects: object_map,
        };

        let result = transform_neo_feed(&feed);
        assert_eq!(result.len(), 1);

        let (asteroid, approaches) = &result[0];
        assert_eq!(asteroid.neo_reference_id, "REF123");
        assert_eq!(asteroid.name, "Asteroid Zero");
        assert_eq!(asteroid.absolute_magnitude, 20.0);

        assert_eq!(approaches.len(), 1);
        let approach = &approaches[0];
        assert_eq!(approach.epoch_date_close_approach, 123456789);
        assert_eq!(
            approach.hazard_classification,
            HazardClassification::Critical
        );
    }
}
