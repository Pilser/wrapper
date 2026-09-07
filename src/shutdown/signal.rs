//! OS signal handling (Ctrl-C) for shutdown.

use super::GracefulShutdown;

/// Block until Ctrl-C (or an earlier [`GracefulShutdown::shutdown`]), then
/// flag `shutdown` for graceful teardown.
pub async fn wait_for_signal(shutdown: &GracefulShutdown) {
    if shutdown.is_shutdown() {
        return;
    }
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = shutdown.wait() => {},
    }
    shutdown.shutdown();
}
