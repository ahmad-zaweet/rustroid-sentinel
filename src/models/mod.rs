//! # Domain Models
//!
//! This module defines the core domain models used throughout the Rustroid Sentinel system.
//! It includes representations for Near-Earth Objects (asteroids) and their specific
//! orbital approaches, as well as the logic for classifying the hazard level of an approach.

pub mod approach;
pub mod asteroid;

pub use approach::Approach;
pub use asteroid::Asteroid;

use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::fmt;

/// Status of an ETL (Extract, Transform, Load) job.
///
/// Represents the current state of a data pipeline execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum EtlStatus {
    /// ETL job is currently running.
    Running,
    /// ETL job completed successfully.
    Completed,
    /// ETL job failed with an error.
    Failed,
}

impl EtlStatus {
    /// Returns true if the status is Running.
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Returns true if the status is Completed.
    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed)
    }

    /// Returns true if the status is Failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed)
    }

    /// Returns a human-readable display string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl std::fmt::Display for EtlStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<&str> for EtlStatus {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Running, // Default fallback
        }
    }
}

/// Represents the threat level of a near-Earth object approach.
///
/// Classification is based on a combination of factors including NASA's Potentially
/// Hazardous Asteroid (PHA) designation, estimated diameter, relative velocity,
/// and miss distance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HazardClassification {
    /// NASA PHA with miss distance < 0.05 AU. Represents an immediate monitoring priority.
    Critical,
    /// Large (>0.5 km) or fast (>20 km/s) with close approach (<0.1 AU).
    High,
    /// Medium-sized (>0.1 km) or moderately fast (>10 km/s).
    Medium,
    /// All other asteroids that do not meet the criteria for higher classification levels.
    Low,
}

impl fmt::Display for HazardClassification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HazardClassification::Critical => write!(f, "Critical"),
            HazardClassification::High => write!(f, "High"),
            HazardClassification::Medium => write!(f, "Medium"),
            HazardClassification::Low => write!(f, "Low"),
        }
    }
}

impl HazardClassification {
    /// Parses a string into a `HazardClassification`.
    /// Falls back to `Low` for unrecognized values.
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "Critical" => Self::Critical,
            "High" => Self::High,
            "Medium" => Self::Medium,
            _ => Self::Low,
        }
    }
}
