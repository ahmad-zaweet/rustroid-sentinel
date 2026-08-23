//! Response types for JPL's Sentry impact-monitoring API
//! (`https://ssd-api.jpl.nasa.gov/sentry.api`).
//!
//! Unlike NeoWs, Sentry encodes essentially all numeric fields as JSON
//! *strings* (e.g. `"ts_max": "0"`, `"ip": "1.6e-05"`), and a query for an
//! object that isn't a current virtual impactor returns an `error` field
//! instead of a `summary` — still with HTTP 200. See
//! `https://ssd-api.jpl.nasa.gov/doc/sentry.html`.

use serde::{Deserialize, Deserializer};

/// Top-level response from a single-object Sentry lookup (`?des=` or `?spk=`).
///
/// Only ever one of `summary`/`error` is present; both are optional so a
/// single struct covers the match and no-match/removed shapes without a
/// custom `Deserialize` impl.
#[derive(Debug, Clone, Deserialize)]
pub struct SentryLookupResponse {
    /// Present when the object is a currently tracked virtual impactor.
    pub summary: Option<SentrySummary>,
    /// Present when the object isn't found or was removed from the Sentry
    /// list (e.g. after an orbit refinement ruled out impact risk).
    pub error: Option<String>,
}

/// The `summary` object of a Sentry single-object response: the aggregate
/// hazard figures across all of that object's potential impacts.
#[derive(Debug, Clone, Deserialize)]
pub struct SentrySummary {
    /// Primary designation, e.g. "99942".
    pub des: String,
    /// Cumulative Palermo Scale across all potential impacts.
    #[serde(deserialize_with = "de_opt_f64_from_str", default)]
    pub ps_cum: Option<f64>,
    /// Maximum Torino Scale across all potential impacts (0-10).
    #[serde(deserialize_with = "de_opt_i16_from_str", default)]
    pub ts_max: Option<i16>,
    /// Cumulative impact probability.
    #[serde(deserialize_with = "de_opt_f64_from_str", default)]
    pub ip: Option<f64>,
}

/// Deserializes a Sentry numeric-as-string field into `Option<f64>`.
/// Missing, `null`, and unparseable values all map to `None` rather than
/// failing the whole response — Sentry's own fields are inconsistently
/// populated (e.g. `diameter` is often absent).
fn de_opt_f64_from_str<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)
        .unwrap_or(None)
        .and_then(|s| s.parse::<f64>().ok()))
}

/// Deserializes a Sentry numeric-as-string field into `Option<i16>`.
fn de_opt_i16_from_str<'de, D>(deserializer: D) -> Result<Option<i16>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)
        .unwrap_or(None)
        .and_then(|s| s.parse::<f64>().ok())
        .map(|f| f.round() as i16))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_matched_response() {
        let json = r#"{
            "signature": {"version": "1.0", "source": "NASA/JPL Sentry Data API"},
            "summary": {
                "des": "99942",
                "fullname": "99942 Apophis (2004 MN4)",
                "ps_cum": "-3.32",
                "ps_max": "-3.32",
                "ts_max": "0",
                "ip": "2.7e-06",
                "n_imp": "3"
            }
        }"#;

        let parsed: SentryLookupResponse = serde_json::from_str(json).unwrap();
        let summary = parsed.summary.expect("summary should be present");
        assert_eq!(summary.des, "99942");
        assert_eq!(summary.ts_max, Some(0));
        assert_eq!(summary.ps_cum, Some(-3.32));
        assert!(parsed.error.is_none());
    }

    #[test]
    fn test_deserialize_not_found_response() {
        let json = r#"{
            "signature": {"version": "1.0", "source": "NASA/JPL Sentry Data API"},
            "error": "specified object not found"
        }"#;

        let parsed: SentryLookupResponse = serde_json::from_str(json).unwrap();
        assert!(parsed.summary.is_none());
        assert_eq!(parsed.error.as_deref(), Some("specified object not found"));
    }

    #[test]
    fn test_deserialize_removed_response() {
        let json = r#"{
            "signature": {"version": "1.0", "source": "NASA/JPL Sentry Data API"},
            "error": "specified object removed",
            "removed": "2021-03-05 12:00:00"
        }"#;

        let parsed: SentryLookupResponse = serde_json::from_str(json).unwrap();
        assert!(parsed.summary.is_none());
        assert_eq!(parsed.error.as_deref(), Some("specified object removed"));
    }

    #[test]
    fn test_deserialize_tolerates_missing_numeric_fields() {
        let json = r#"{
            "summary": {
                "des": "2010 XZ",
                "ts_max": "0",
                "ip": "1e-08"
            }
        }"#;

        let parsed: SentryLookupResponse = serde_json::from_str(json).unwrap();
        let summary = parsed.summary.unwrap();
        assert_eq!(summary.ps_cum, None);
        assert_eq!(summary.ts_max, Some(0));
    }
}
