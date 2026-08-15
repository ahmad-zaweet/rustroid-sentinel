//! # Hazard Event Stream
//!
//! Publishes newly-loaded hazardous approaches to a broadcast channel so the
//! SSE endpoint (`/api/events/hazards`) and the internal ingest endpoint
//! (`POST /internal/events`) can share a single fan-out.
//!
//! `PgListenSource` (behind the `pg-listen` feature) is an alternative
//! producer that forwards `LISTEN`/`NOTIFY` payloads into the same channel;
//! it is not a separate consumer-facing abstraction, since every consumer
//! subscribes to the same [`tokio::sync::broadcast::Sender`] regardless of
//! how events are produced.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::models::HazardClassification;

/// Default capacity of the broadcast channel buffer.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// A hazardous approach event published to SSE subscribers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HazardEvent {
    /// The `Approach` UUID this event describes.
    pub approach_id: Uuid,
    /// Human-readable asteroid name, for display without a follow-up lookup.
    pub asteroid_name: String,
    /// The computed hazard classification for this approach.
    pub hazard_classification: HazardClassification,
    /// When the event was published.
    pub timestamp: DateTime<Utc>,
}

/// Creates a fresh broadcast channel for hazard events.
///
/// The sender is cloned into `AppState` and into any background producer
/// (e.g. `PgListenSource`); each SSE connection calls `.subscribe()` on it
/// to get its own receiver.
pub fn channel() -> broadcast::Sender<HazardEvent> {
    let (tx, _rx) = broadcast::channel(DEFAULT_CHANNEL_CAPACITY);
    tx
}

#[cfg(feature = "pg-listen")]
pub mod pg_listen;
