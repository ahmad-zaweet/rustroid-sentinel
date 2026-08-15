//! SSE endpoint streaming hazard events to connected dashboards.

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::stream::unfold;
use tokio::sync::broadcast;

use crate::server::AppState;

/// How often a keep-alive comment is sent to stop free-tier proxies from
/// cutting idle connections.
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

/// Decrements the shared subscriber count when the SSE stream is dropped
/// (client disconnect, server shutdown, or the stream running to completion).
struct SubscriberGuard {
    count: Arc<AtomicUsize>,
}

impl Drop for SubscriberGuard {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::SeqCst);
    }
}

/// GET /api/events/hazards
///
/// Streams [`HazardEvent`](crate::events::HazardEvent)s as they're published.
/// Rejects new connections with 503 once `max_hazard_subscribers` are
/// already attached — each stream holds a task, so the cap bounds worst-case
/// resource use on a small deployment.
///
/// A subscriber that falls behind the broadcast buffer receives a typed
/// `lagged` event carrying the number of skipped messages instead of a
/// silent gap or a closed connection; the client is expected to re-fetch
/// `/api/approaches` to resynchronize. Server-side replay via
/// `Last-Event-ID` is out of scope.
pub async fn hazard_events_stream(State(state): State<AppState>) -> Response {
    let max_subscribers = state.config.max_hazard_subscribers;

    loop {
        let current = state.hazard_subscriber_count.load(Ordering::SeqCst);
        if current >= max_subscribers {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        if state
            .hazard_subscriber_count
            .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            break;
        }
    }

    let guard = SubscriberGuard {
        count: state.hazard_subscriber_count.clone(),
    };
    let rx = state.events_tx.subscribe();

    let stream = unfold((guard, rx), |(guard, mut rx)| async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let sse_event = match Event::default().event("hazard").json_data(&event) {
                        Ok(sse_event) => sse_event,
                        Err(error) => {
                            tracing::error!(%error, "Failed to serialize hazard event for SSE");
                            continue;
                        }
                    };
                    return Some((Ok::<_, Infallible>(sse_event), (guard, rx)));
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    let sse_event = Event::default().event("lagged").data(skipped.to_string());
                    return Some((Ok::<_, Infallible>(sse_event), (guard, rx)));
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(KEEP_ALIVE_INTERVAL))
        .into_response()
}
