//! A watch-channel based shutdown flag shared by subsystems.

use tokio::sync::watch;

/// Coordinates graceful shutdown across tasks.
#[derive(Debug, Clone)]
pub struct GracefulShutdown {
    tx: watch::Sender<bool>,
}

impl Default for GracefulShutdown {
    fn default() -> Self {
        Self::new()
    }
}

impl GracefulShutdown {
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(false);
        Self { tx }
    }

    /// Signal every waiter to shut down.
    pub fn shutdown(&self) {
        let _ = self.tx.send(true);
    }

    /// Whether shutdown has been requested.
    pub fn is_shutdown(&self) -> bool {
        *self.tx.borrow()
    }

    /// Wait until [`shutdown`](Self::shutdown) is called.
    pub async fn wait(&self) {
        let mut rx = self.tx.subscribe();
        let _ = rx.wait_for(|flag| *flag).await;
    }
}
