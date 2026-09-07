//! Boot sequence for a microVM.

use shared::{Error as CoreError, VmTemplate};
use telemetry::{Level, Telemetry};

use crate::firecracker::boot_source::boot_source;
use crate::firecracker::config::MachineConfig;
use crate::firecracker::drive::root_drive;
use crate::firecracker::FirecrackerClient;

/// Boot a VM: PUT machine-config + boot-source + drives, then start the instance.
pub async fn boot_vm(
    telemetry: &Telemetry,
    client: &FirecrackerClient,
    tenant_id: &str,
    template: &VmTemplate,
) -> shared::Result<()> {
    telemetry
        .builder()
        .level(Level::Info)
        .kind("vm.boot.started")
        .category("vm")
        .tenant_id(Some(tenant_id.to_string()))
        .data("template", template.id.clone())
        .message("boot started")
        .emit();

    let machine = MachineConfig::new(template.vcpu_count, template.memory_mb);
    let boot_cfg = boot_source(&template.kernel_path);
    let root = root_drive(&template.rootfs_path);

    let result = boot_vm_inner(client, &machine, &boot_cfg, &root).await;

    match result {
        Ok(()) => {
            telemetry
                .builder()
                .level(Level::Info)
                .kind("vm.boot.completed")
                .category("vm")
                .tenant_id(Some(tenant_id.to_string()))
                .message("boot completed")
                .emit();
            Ok(())
        }
        Err(e) => {
            telemetry
                .builder()
                .level(Level::Error)
                .kind("vm.boot.failed")
                .category("vm")
                .tenant_id(Some(tenant_id.to_string()))
                .message(format!("boot failed: {e}"))
                .emit();
            Err(e)
        }
    }
}

async fn boot_vm_inner(
    client: &FirecrackerClient,
    machine: &MachineConfig,
    boot_cfg: &crate::firecracker::BootSourceConfig,
    root: &crate::firecracker::config::DriveConfig,
) -> shared::Result<()> {
    let machine_body =
        serde_json::to_value(machine).map_err(|e| CoreError::Vm(e.to_string()))?;
    client
        .put("/machine-config", &machine_body)
        .await
        .map_err(|e| CoreError::Vm(e.to_string()))?;
    let boot_body =
        serde_json::to_value(boot_cfg).map_err(|e| CoreError::Vm(e.to_string()))?;
    client
        .put("/boot-source", &boot_body)
        .await
        .map_err(|e| CoreError::Vm(e.to_string()))?;
    let drive_body =
        serde_json::to_value(root).map_err(|e| CoreError::Vm(e.to_string()))?;
    let drive_path = format!("/drives/{}", root.drive_id);
    client
        .put(&drive_path, &drive_body)
        .await
        .map_err(|e| CoreError::Vm(e.to_string()))?;
    client
        .put_action("InstanceStart")
        .await
        .map_err(|e| CoreError::Vm(e.to_string()))?;
    Ok(())
}

pub async fn boot_completed(telemetry: &Telemetry) {
    telemetry.builder().level(Level::Info).kind("vm.boot.completed").category("vm")
        .message("boot completed").emit();
}
