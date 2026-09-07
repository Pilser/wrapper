//! Graceful-shutdown signal for the proxy server.

use tokio::sync::watch;

/// Cloneable shutdown trigger; [`GracefulShutdown::wait`] resolves on trigger.
#[derive(Clone, Debug)]
pub struct GracefulShutdown {
    tx: watch::Sender<bool>,
    rx: watch::Receiver<bool>,
}

impl GracefulShutdown {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self { tx, rx }
    }

    pub fn trigger(&self) {
        let _ = self.tx.send(true);
    }

    pub fn is_shutdown(&self) -> bool {
        *self.rx.borrow()
    }

    pub async fn wait(&mut self) {
        while self.rx.changed().await.is_ok() {
            if *self.rx.borrow() {
                break;
            }
        }
    }
}

impl Default for GracefulShutdown {
    fn default() -> Self {
        Self::new()
    }
}
