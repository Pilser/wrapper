//! `vm` — Docker tenant lifecycle management and templates.

pub mod docker;
pub mod idle;
pub mod resources;
pub mod runtime;
pub mod templates;

pub use docker::{ContainerStats, DockerDriver};
pub use idle::{IdleAction, IdleTracker};
pub use runtime::{VmRuntime, ZERO_WRAPPER_PORT};
pub use templates::{TemplateRegistry, VmTemplateBuilder};

use shared::VmInfo;
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
