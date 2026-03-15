//! # Graceful Shutdown
//!
//! This module handles graceful shutdown signal handling.

use tokio::signal;
use tracing::{error, info};

/// Waits for a shutdown signal (SIGINT or SIGTERM).
///
/// This function installs signal handlers for Ctrl+C and SIGTERM, then waits
/// for either signal. If signal handler installation fails, logs an error and
/// waits indefinitely (requiring force-kill to terminate).
pub async fn shutdown_signal() {
    let ctrl_c = async {
        match signal::ctrl_c().await {
            Ok(()) => (),
            Err(err) => {
                error!(error = %err, "Failed to install Ctrl+C handler; graceful shutdown disabled");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(err) => {
                error!(error = %err, "Failed to install SIGTERM handler; graceful shutdown disabled");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        }
        _ = terminate => {
            info!("Received SIGTERM signal");
        }
    }
}
