//! Graceful shutdown of a microVM.

use telemetry::{Level, Telemetry};

use crate::firecracker::FirecrackerClient;

pub async fn shutdown_vm(
    telemetry: &Telemetry,
    client: &FirecrackerClient,
) -> core::Result<()> {
    telemetry.builder().level(Level::Info).kind("vm.shutdown.started").category("vm")
        .message("shutdown started").emit();
    let result = client
        .put_action("SendCtrlAltDel")
        .await
        .map_err(|e| core::Error::Vm(e.to_string()));
    match result {
        Ok(_) => {
            telemetry.builder().level(Level::Info).kind("vm.shutdown.completed").category("vm")
                .message("shutdown completed").emit();
            Ok(())
        }
        Err(e) => {
            telemetry.builder().level(Level::Error).kind("vm.shutdown.failed").category("vm")
                .message(format!("shutdown failed: {e}")).emit();
            Err(e)
        }
    }
}
