//! Internal hazard-event ingest endpoint.
//!
//! `POST /internal/events` is the webhook the ETL job (and, if enabled, the
//! `pg-listen` background task) uses to publish [`HazardEvent`]s that the
//! `/api/events/hazards` SSE endpoint fans out to browsers. It is registered
//! outside the public `/api` router, is not linked from the dashboard, and is
//! gated by a shared secret compared in constant time.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};

use crate::events::HazardEvent;
use crate::server::AppState;

/// Maximum accepted body size, tighter than the 1 MB global request limit
/// (`src/server/router.rs`) since a batch of events is small.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

const TOKEN_HEADER: &str = "x-internal-token";

/// Compares two byte strings without leaking timing information about where
/// they first differ. Only the compared bytes are constant-time; a length
/// mismatch still short-circuits, which leaks length but not content.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// POST /internal/events
///
/// Authenticates via `X-Internal-Token`, publishes each event in the batch to
/// the shared broadcast channel, and returns 202 regardless of whether any
/// SSE clients are currently connected.
pub async fn ingest_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(events): Json<Vec<HazardEvent>>,
) -> StatusCode {
    let provided = headers
        .get(TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();

    if !constant_time_eq(provided.as_bytes(), state.internal_event_token.as_bytes()) {
        tracing::warn!("Rejected /internal/events request: invalid or missing token");
        return StatusCode::UNAUTHORIZED;
    }

    for event in events {
        // Ignore SendError: no receivers just means no SSE clients right now.
        let _ = state.events_tx.send(event);
    }

    StatusCode::ACCEPTED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches_equal_slices() {
        assert!(constant_time_eq(b"secret-token", b"secret-token"));
    }

    #[test]
    fn constant_time_eq_rejects_different_content() {
        assert!(!constant_time_eq(b"secret-token", b"wrong-token!"));
    }

    #[test]
    fn constant_time_eq_rejects_different_length() {
        assert!(!constant_time_eq(b"short", b"much-longer-value"));
    }
}
