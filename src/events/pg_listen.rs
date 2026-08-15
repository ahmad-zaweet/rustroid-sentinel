//! # Postgres `LISTEN`/`NOTIFY` event source
//!
//! Alternative to the webhook-only ingest path (`POST /internal/events`):
//! listens on the **direct** (non-pooler) Postgres connection
//! (`DatabaseConfig.listen_url`) and forwards `NOTIFY` payloads into the same
//! [`broadcast::Sender`] used everywhere else. Neon's pooled endpoint is
//! PgBouncer in transaction mode, which cannot `LISTEN` — this must run
//! against `listen_url`, never the pooled `url`.

use sqlx::postgres::PgListener;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use super::HazardEvent;

/// Postgres channel name that `NOTIFY` payloads are published on.
pub const CHANNEL: &str = "hazard_events";

/// Connects to `listen_url`, subscribes to [`CHANNEL`], and forwards every
/// notification payload (expected to be JSON-encoded [`HazardEvent`]) into
/// `tx` for the lifetime of the process.
///
/// Returns an error immediately if the initial connection or `LISTEN` fails,
/// so callers can fail startup loudly rather than silently run without
/// events. Once connected, malformed individual payloads are logged and
/// skipped rather than tearing down the listener.
pub async fn run(listen_url: &str, tx: broadcast::Sender<HazardEvent>) -> Result<(), sqlx::Error> {
    let mut listener = connect_and_listen(listen_url).await?;
    info!(channel = CHANNEL, "PgListenSource subscribed");

    loop {
        let notification = match listener.recv().await {
            Ok(notification) => notification,
            Err(error) => {
                error!(%error, "PgListenSource lost connection, reconnecting");
                listener = connect_and_listen(listen_url).await?;
                continue;
            }
        };

        forward_payload(notification.payload(), &tx);
    }
}

/// Connects to `listen_url` and subscribes to [`CHANNEL`], for both the
/// initial connection and post-error reconnects.
async fn connect_and_listen(listen_url: &str) -> Result<PgListener, sqlx::Error> {
    let mut listener = PgListener::connect(listen_url).await?;
    listener.listen(CHANNEL).await?;
    Ok(listener)
}

/// Decodes a single `NOTIFY` payload and forwards it to `tx`. Malformed
/// payloads are logged and dropped rather than tearing down the listener.
fn forward_payload(payload: &str, tx: &broadcast::Sender<HazardEvent>) {
    match serde_json::from_str::<HazardEvent>(payload) {
        Ok(event) => {
            // No subscribers is not an error - the SSE endpoint may simply
            // have no connected clients right now.
            let _ = tx.send(event);
        }
        Err(error) => {
            warn!(%error, payload, "Skipping malformed hazard event payload");
        }
    }
}
