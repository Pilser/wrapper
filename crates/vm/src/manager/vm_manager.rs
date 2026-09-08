//! The VM manager: boot/teardown orchestration.

use shared::VmTemplate;
use telemetry::Telemetry;

use crate::firecracker::FirecrackerClient;
use crate::lifecycle::boot::boot_vm;

use super::registry::VmRegistry;

/// Manages the lifecycle of all running VMs.
#[derive(Clone)]
pub struct VmManager {
    pub registry: VmRegistry,
    pub telemetry: Telemetry,
    pub firecracker_bin: String,
}

impl VmManager {
    pub fn new(firecracker_bin: impl Into<String>) -> Self {
        Self {
            registry: VmRegistry::new(),
            telemetry: Telemetry::noop(),
            firecracker_bin: firecracker_bin.into(),
        }
    }

    pub fn with_telemetry(firecracker_bin: impl Into<String>, telemetry: Telemetry) -> Self {
        Self { registry: VmRegistry::new(), telemetry, firecracker_bin: firecracker_bin.into() }
    }

    /// Boot a VM for `tenant_id` from `template`.
    ///
    /// 1. Spawn the Firecracker process (creates the API socket).
    /// 2. Configure boot-source (kernel + initramfs), machine-config.
    /// 3. Start the instance.
    pub async fn boot(&self, tenant_id: &str, template: &VmTemplate) -> anyhow::Result<()> {
        let sock = std::path::PathBuf::from(format!("/tmp/fc_{tenant_id}.sock"));
        // 1. Spawn Firecracker and wait for its API socket.
        let client = FirecrackerClient::spawn_firecracker(&self.firecracker_bin, &sock).await?;
        // 2 + 3. Configure and start.
        boot_vm(&self.telemetry, &client, tenant_id, template)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(())
    }

    pub async fn shutdown(&self, tenant_id: &str) -> anyhow::Result<()> {
        self.registry.set(tenant_id, "stopped");
        Ok(())
    }
}
