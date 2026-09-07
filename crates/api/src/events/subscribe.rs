//! Subscribe to the live telemetry stream from plugins.

use telemetry::{Subscription, Telemetry};

/// Subscribe to live events (same stream the dashboard uses).
pub fn subscribe(telemetry: &Telemetry) -> Subscription {
    telemetry.subscribe()
}
