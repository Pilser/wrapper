//! `admin` — staff-facing management: categories, VM templates, deployments,
//! tenants, staff accounts and dashboard aggregates.
//!
//! Plugin-first: every operation emits a telemetry event so plugins can
//! observe or react to admin activity. Operations persist through the `db`
//! crate's global-table helpers (`categories`, `vm_templates`, `tenants`,
//! `sessions`, `networks`, `category_assignments`, `deployment_configs`,
//! `staff`).

pub mod categories;
pub mod dashboard;
pub mod deployments;
pub mod staff;
pub mod tenants;
pub mod vm_templates;

use telemetry::Telemetry;

/// Facade over the admin subsystem.
#[derive(Clone, Debug)]
pub struct AdminService {
    pub telemetry: Telemetry,
}

impl AdminService {
    pub fn new(telemetry: Telemetry) -> Self {
        Self { telemetry }
    }

    pub fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }
}

/// Build the admin service (startup flow step 12: load categories and
/// deployment configs).
pub fn init(telemetry: Telemetry) -> AdminService {
    telemetry
        .builder()
        .level(telemetry::Level::Info)
        .kind("system.startup")
        .category("admin")
        .source("admin::init")
        .message("admin subsystem initialized")
        .tag("admin")
        .emit();
    AdminService::new(telemetry)
}
