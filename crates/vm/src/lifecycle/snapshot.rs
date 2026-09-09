//! Snapshot a running VM: pause vCPUs first (Firecracker REQUIRES Paused
//! state for `/snapshot/create`), then write state + mem files. No resume:
//! the idle path kills the VM right after.

use telemetry::{Level, Telemetry};

use crate::firecracker::FirecrackerClient;

pub async fn snapshot_vm(
    telemetry: &Telemetry,
    client: &FirecrackerClient,
    vm_id: &str,
    snapshot_path: &str,
    mem_file_path: &str,
) -> shared::Result<()> {
    telemetry.builder().level(Level::Info).kind("vm.snapshot.started").category("vm")
        .vm_id(Some(vm_id.to_string()))
        .message("snapshot started").emit();
    if let Err(e) = client.pause_vm().await {
        return Err(shared::Error::Vm(format!("pause before snapshot: {e}")));
    }
    let result = client
        .create_snapshot(snapshot_path, mem_file_path)
        .await
        .map_err(|e| shared::Error::Vm(e.to_string()));
    match result {
        Ok(_) => {
            telemetry.builder().level(Level::Info).kind("vm.snapshot.completed").category("vm")
                .vm_id(Some(vm_id.to_string()))
                .message("snapshot completed").emit();
            Ok(())
        }
        Err(e) => {
            telemetry.builder().level(Level::Error).kind("vm.snapshot.failed").category("vm")
                .vm_id(Some(vm_id.to_string()))
                .message(format!("snapshot failed: {e}")).emit();
            Err(e)
        }
    }
}
