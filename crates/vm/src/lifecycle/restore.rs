//! Restore a VM from a snapshot via `PUT /snapshot/load`.

use telemetry::{Level, Telemetry};

use crate::firecracker::FirecrackerClient;

pub async fn restore_vm(
    telemetry: &Telemetry,
    client: &FirecrackerClient,
    vm_id: &str,
    snapshot_path: &str,
    mem_file_path: &str,
    resume: bool,
) -> shared::Result<()> {
    telemetry.builder().level(Level::Info).kind("vm.restore.started").category("vm")
        .vm_id(Some(vm_id.to_string()))
        .message("restore started").emit();
    let result = client
        .load_snapshot(snapshot_path, mem_file_path, resume)
        .await
        .map_err(|e| shared::Error::Vm(e.to_string()));
    match result {
        Ok(_) => {
            telemetry.builder().level(Level::Info).kind("vm.restore.completed").category("vm")
                .vm_id(Some(vm_id.to_string()))
                .message("restore completed").emit();
            Ok(())
        }
        Err(e) => {
            telemetry.builder().level(Level::Error).kind("vm.restore.failed").category("vm")
                .vm_id(Some(vm_id.to_string()))
                .message(format!("restore failed: {e}")).emit();
            Err(e)
        }
    }
}
