//! Periodic flush scheduling for the file writer.

use tokio::time::{self, Duration};

/// Runs a periodic flush that invokes `flush` every `interval_seconds`.
///
/// This is used to bound the time buffered events sit in memory when the
/// buffer is not yet full.
pub async fn periodic_flush(
    interval_seconds: u64,
    mut flush: impl FnMut() -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<()> {
    let mut interval = time::interval(Duration::from_secs(interval_seconds.max(1)));
    loop {
        interval.tick().await;
        if let Err(e) = flush() {
            // Swallow transient errors; the loop keeps trying.
            let _ = e;
        }
    }
}