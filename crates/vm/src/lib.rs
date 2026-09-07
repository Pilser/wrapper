//! `vm` — Firecracker VM lifecycle management and templates.

pub mod firecracker;
pub mod idle;
pub mod lifecycle;
pub mod manager;
pub mod resources;
pub mod templates;

pub use firecracker::FirecrackerClient;
pub use idle::{IdleAction, IdleTracker};
pub use manager::{VmManager, VmRegistry};
pub use templates::{TemplateRegistry, VmTemplateBuilder};

use core::VmInfo;
use telemetry::Telemetry;

/// Facade over VM management with shared telemetry.
#[derive(Clone)]
pub struct VmService {
    pub telemetry: Telemetry,
}

impl VmService {
    pub fn new(telemetry: Telemetry) -> Self {
        Self { telemetry }
    }

    pub fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }

    pub fn health(&self) -> bool {
        true
    }

    pub fn from_info(_info: &VmInfo) -> VmService {
        VmService::new(Telemetry::noop())
    }
}
