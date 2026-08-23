//! Response types for JPL's Small-Body Database (SBDB) API
//! (`https://ssd-api.jpl.nasa.gov/sbdb.api`), used to fetch orbital elements.
//!
//! Unlike Sentry, a query for an object that isn't found returns a `message`
//! field and omits `object`/`orbit` entirely — still with HTTP 200. Orbital
//! elements come back as a name/value array rather than fixed fields (each
//! element also carries units, uncertainty, etc., which we don't need), so
//! extraction happens by name lookup rather than a direct field mapping. See
//! `https://ssd-api.jpl.nasa.gov/doc/sbdb.html`.

use serde::Deserialize;

/// Top-level response from a single-object SBDB lookup (`?spk=`).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SbdbResponse {
    /// Present when the object isn't found, e.g. "specified object was not found".
    #[serde(default)]
    pub message: Option<String>,
    /// Present when the object is found. Carries the orbit classification.
    #[serde(default)]
    pub object: Option<SbdbObjectInfo>,
    /// Present when the object is found. Carries the orbital elements.
    #[serde(default)]
    pub orbit: Option<SbdbOrbit>,
    /// Physical parameters (albedo, spectral class, etc.), requested via
    /// `phys-par=1`. Sparse — most fields are absent for most objects.
    #[serde(default)]
    pub phys_par: Option<Vec<SbdbPhysParam>>,
}

/// Metadata about the object itself, as opposed to its orbit.
#[derive(Debug, Clone, Deserialize)]
pub struct SbdbObjectInfo {
    /// The object's dynamical orbit classification (e.g. "Aten", "Amor").
    #[serde(default)]
    pub orbit_class: Option<SbdbOrbitClass>,
}

/// An object's dynamical orbit classification.
#[derive(Debug, Clone, Deserialize)]
pub struct SbdbOrbitClass {
    /// Human-readable class name, e.g. "Aten".
    pub name: String,
}

/// The `orbit` object of an SBDB response.
#[derive(Debug, Clone, Deserialize)]
pub struct SbdbOrbit {
    /// The fitted orbital elements, as a name/value array.
    pub elements: Vec<SbdbElement>,
}

/// One orbital element, e.g. `{"name": "e", "value": "0.191"}`. SBDB encodes
/// every numeric field as a JSON string, same as Sentry.
#[derive(Debug, Clone, Deserialize)]
pub struct SbdbElement {
    /// SBDB's short element name, e.g. `"e"`, `"a"`, `"i"`.
    pub name: String,
    /// The element's value, encoded as a string.
    pub value: String,
}

/// One physical parameter, e.g. `{"name": "albedo", "value": "0.42"}`.
/// `value` is nullable in the API for parameters that are listed but unmeasured.
#[derive(Debug, Clone, Deserialize)]
pub struct SbdbPhysParam {
    /// SBDB's parameter name, e.g. `"albedo"`, `"spec_T"`.
    pub name: String,
    /// The parameter's value, encoded as a string, or `None` if unmeasured.
    pub value: Option<String>,
}

impl SbdbOrbit {
    /// Looks up an orbital element by its SBDB short name (e.g. `"e"`,
    /// `"a"`, `"i"`, `"om"`, `"w"`, `"ma"`, `"per"`) and parses it as `f64`.
    /// Missing or unparseable elements map to `None` rather than failing the
    /// whole lookup — SBDB's element set isn't guaranteed complete for every
    /// object (e.g. some poorly-observed objects lack a period fit).
    pub fn element_f64(&self, name: &str) -> Option<f64> {
        self.elements
            .iter()
            .find(|e| e.name == name)
            .and_then(|e| e.value.parse::<f64>().ok())
    }
}

/// Extracted, typed summary of the fields we persist from an SBDB response.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SbdbOrbitSummary {
    /// Orbital eccentricity (dimensionless).
    pub eccentricity: Option<f64>,
    /// Semi-major axis, in AU.
    pub semi_major_axis_au: Option<f64>,
    /// Inclination to the ecliptic, in degrees.
    pub inclination_deg: Option<f64>,
    /// Longitude of the ascending node, in degrees.
    pub ascending_node_deg: Option<f64>,
    /// Argument of perihelion, in degrees.
    pub perihelion_arg_deg: Option<f64>,
    /// Mean anomaly, in degrees.
    pub mean_anomaly_deg: Option<f64>,
    /// Orbital period, in days.
    pub orbital_period_days: Option<f64>,
    /// Dynamical orbit classification, e.g. "Aten".
    pub orbit_class: Option<String>,
    /// Spectral class (Tholen, falling back to SMASS-II), e.g. "Sq".
    pub spectral_class: Option<String>,
    /// Geometric albedo (dimensionless).
    pub albedo: Option<f64>,
}

impl SbdbResponse {
    /// Extracts the fields we persist, or `None` if this response has no
    /// `orbit` (object not found, or found but orbit-less — shouldn't happen
    /// for a numbered/designated NEO, but the API doesn't guarantee it).
    pub fn to_orbit_summary(&self) -> Option<SbdbOrbitSummary> {
        let orbit = self.orbit.as_ref()?;

        let orbit_class = self
            .object
            .as_ref()
            .and_then(|o| o.orbit_class.as_ref())
            .map(|c| c.name.clone());

        let phys_par = self.phys_par.as_deref().unwrap_or(&[]);
        let find_phys = |name: &str| {
            phys_par
                .iter()
                .find(|p| p.name == name)
                .and_then(|p| p.value.as_deref())
        };
        let spectral_class = find_phys("spec_T")
            .or_else(|| find_phys("spec_B"))
            .map(str::to_string);
        let albedo = find_phys("albedo").and_then(|v| v.parse::<f64>().ok());

        Some(SbdbOrbitSummary {
            eccentricity: orbit.element_f64("e"),
            semi_major_axis_au: orbit.element_f64("a"),
            inclination_deg: orbit.element_f64("i"),
            ascending_node_deg: orbit.element_f64("om"),
            perihelion_arg_deg: orbit.element_f64("w"),
            mean_anomaly_deg: orbit.element_f64("ma"),
            orbital_period_days: orbit.element_f64("per"),
            orbit_class,
            spectral_class,
            albedo,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_matched_response() {
        let json = r#"{
            "object": {
                "fullname": "99942 Apophis (2004 MN4)",
                "orbit_class": {"name": "Aten", "code": "ATE"}
            },
            "orbit": {
                "elements": [
                    {"name": "e", "value": "0.1914"},
                    {"name": "a", "value": "0.9224"},
                    {"name": "i", "value": "3.3412"},
                    {"name": "om", "value": "203.9932"},
                    {"name": "w", "value": "126.6"},
                    {"name": "ma", "value": "245.9"},
                    {"name": "per", "value": "323.6"}
                ]
            },
            "phys_par": [
                {"name": "albedo", "value": "0.24"},
                {"name": "spec_T", "value": "Sq"}
            ]
        }"#;

        let parsed: SbdbResponse = serde_json::from_str(json).unwrap();
        let summary = parsed
            .to_orbit_summary()
            .expect("expected an orbit summary");
        assert_eq!(summary.eccentricity, Some(0.1914));
        assert_eq!(summary.semi_major_axis_au, Some(0.9224));
        assert_eq!(summary.orbit_class.as_deref(), Some("Aten"));
        assert_eq!(summary.spectral_class.as_deref(), Some("Sq"));
        assert_eq!(summary.albedo, Some(0.24));
    }

    #[test]
    fn test_deserialize_not_found_response() {
        let json = r#"{"message": "specified object was not found"}"#;

        let parsed: SbdbResponse = serde_json::from_str(json).unwrap();
        assert!(parsed.orbit.is_none());
        assert!(parsed.to_orbit_summary().is_none());
        assert_eq!(
            parsed.message.as_deref(),
            Some("specified object was not found")
        );
    }

    #[test]
    fn test_deserialize_tolerates_missing_phys_par() {
        let json = r#"{
            "object": {"orbit_class": {"name": "Amor", "code": "AMO"}},
            "orbit": {"elements": [{"name": "e", "value": "0.4"}]}
        }"#;

        let parsed: SbdbResponse = serde_json::from_str(json).unwrap();
        let summary = parsed.to_orbit_summary().unwrap();
        assert_eq!(summary.eccentricity, Some(0.4));
        assert_eq!(summary.albedo, None);
        assert_eq!(summary.spectral_class, None);
    }
}
