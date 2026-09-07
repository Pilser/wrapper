//! VM migration via snapshot-on-source + restore-on-target.

use telemetry::{Level, Telemetry};

use crate::firecracker::FirecrackerClient;
use crate::lifecycle::restore::restore_vm;
use crate::lifecycle::snapshot::snapshot_vm;

pub async fn migrate_vm(
    telemetry: &Telemetry,
    source_client: &FirecrackerClient,
    target_client: &FirecrackerClient,
    vm_id: &str,
    snapshot_path: &str,
    mem_file_path: &str,
) -> shared::Result<()> {
    telemetry.builder().level(Level::Info).kind("vm.migrate.started").category("vm")
        .vm_id(Some(vm_id.to_string()))
        .message("migrate started").emit();
    let result = migrate_inner(
        telemetry,
        source_client,
        target_client,
        vm_id,
        snapshot_path,
        mem_file_path,
    )
    .await;
    match result {
        Ok(()) => {
            telemetry.builder().level(Level::Info).kind("vm.migrate.completed").category("vm")
                .vm_id(Some(vm_id.to_string()))
                .message("migrate completed").emit();
            Ok(())
        }
        Err(e) => {
            telemetry.builder().level(Level::Error).kind("vm.migrate.failed").category("vm")
                .vm_id(Some(vm_id.to_string()))
                .message(format!("migrate failed: {e}")).emit();
            Err(e)
        }
    }
}

async fn migrate_inner(
    telemetry: &Telemetry,
    source_client: &FirecrackerClient,
    target_client: &FirecrackerClient,
    vm_id: &str,
    snapshot_path: &str,
    mem_file_path: &str,
) -> shared::Result<()> {
    snapshot_vm(telemetry, source_client, vm_id, snapshot_path, mem_file_path).await?;
    restore_vm(telemetry, target_client, vm_id, snapshot_path, mem_file_path).await?;
    Ok(())
}
