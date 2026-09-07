//! Oversight reports for staff dashboards (stub).

use shared::now_rfc3339;
use telemetry::{Level, Telemetry};

/// A point-in-time oversight summary.
#[derive(Debug, Clone)]
pub struct OversightReport {
    pub generated_at: String,
    pub summary: String,
}

/// Build an (initially empty) oversight report and emit telemetry.
pub async fn generate_oversight_report(telemetry: &Telemetry) -> OversightReport {
    telemetry
        .builder()
        .level(Level::Info)
        .kind("admin.staff.updated")
        .category("admin")
        .source("admin::staff::oversight")
        .message("oversight report generated")
        .tag("admin")
        .tag("staff")
        .emit();
    OversightReport {
        generated_at: now_rfc3339(),
        summary: "oversight report: no data sources wired yet".to_string(),
    }
}
