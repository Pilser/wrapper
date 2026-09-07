//! Reboot a microVM via the Firecracker reboot action.

use telemetry::{Level, Telemetry};

use crate::firecracker::FirecrackerClient;

pub async fn reboot_vm(
    telemetry: &Telemetry,
    client: &FirecrackerClient,
    tenant_id: &str,
) -> core::Result<()> {
    telemetry.builder().level(Level::Info).kind("vm.reboot.started").category("vm")
        .tenant_id(Some(tenant_id.to_string()))
        .message("reboot started").emit();
    let result = client
        .put_action("SendCtrlAltDel")
        .await
        .map_err(|e| core::Error::Vm(e.to_string()));
    match result {
        Ok(_) => {
            telemetry.builder().level(Level::Info).kind("vm.reboot.completed").category("vm")
                .tenant_id(Some(tenant_id.to_string()))
                .message("reboot completed").emit();
            Ok(())
        }
        Err(e) => {
            telemetry.builder().level(Level::Error).kind("vm.reboot.failed").category("vm")
                .tenant_id(Some(tenant_id.to_string()))
                .message(format!("reboot failed: {e}")).emit();
            Err(e)
        }
    }
}
