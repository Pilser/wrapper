//! VM handle for plugins (delegates to the `vm` crate).

/// Thin wrapper exposing VM management to plugins.
#[derive(Clone, Debug)]
pub struct VmApi {
    pub telemetry: telemetry::Telemetry,
}

impl VmApi {
    pub fn new(telemetry: telemetry::Telemetry) -> Self {
        Self { telemetry }
    }

    /// Build the underlying [`vm::VmService`].
    pub fn service(&self) -> vm::VmService {
        vm::VmService::new(self.telemetry.clone())
    }

    /// Whether the VM subsystem reports healthy.
    pub fn health(&self) -> bool {
        self.service().health()
    }
}
