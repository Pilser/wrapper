//! Admin dashboard aggregates: stats, health and alerts.

pub mod alerts;
pub mod health;
pub mod stats;

pub use alerts::{list_alerts, raise_alert, Alert};
pub use health::{check_health, HealthStatus};
pub use stats::{collect_stats, SystemStats};
