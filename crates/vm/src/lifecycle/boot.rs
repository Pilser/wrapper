//! Boot sequence for a microVM.

use shared::{Error as CoreError, VmTemplate};
use telemetry::{Level, Telemetry};

use crate::firecracker::boot_source::{boot_source, boot_source_with_net};
use crate::firecracker::config::MachineConfig;
use crate::firecracker::drive::root_drive;
use crate::firecracker::network::{gateway_for_ip, network_interface};
use crate::firecracker::FirecrackerClient;

/// Network attachment for a boot: the host tap device backing the guest's
/// `eth0`, the guest MAC, and the IP the guest must configure. When `None`
/// the VM boots with no virtual NIC (initramfs-only, unreachable — kept for
/// backwards compatibility only).
#[derive(Debug, Clone)]
pub struct NetAttach {
    pub tap_name: String,
    pub guest_mac: String,
    pub vm_ip: String,
    pub gateway: String,
}

impl NetAttach {
    pub fn new(
        tap_name: impl Into<String>,
        guest_mac: impl Into<String>,
        vm_ip: impl Into<String>,
    ) -> Self {
        let vm_ip = vm_ip.into();
        let gateway = gateway_for_ip(&vm_ip);
        Self {
            tap_name: tap_name.into(),
            guest_mac: guest_mac.into(),
            vm_ip,
            gateway,
        }
    }
}

/// Boot a VM: PUT machine-config + boot-source + drives, then start the instance.
pub async fn boot_vm(
    telemetry: &Telemetry,
    client: &FirecrackerClient,
    tenant_id: &str,
    template: &VmTemplate,
) -> shared::Result<()> {
    boot_vm_with_net(telemetry, client, tenant_id, template, None).await
}

/// Boot a VM with a virtual NIC attached: PUT machine-config + boot-source
/// (with `vm_ip=`/`gw=` kernel args) + PUT /network-interfaces/eth0 +
/// drives, then start the instance.
pub async fn boot_vm_with_net(
    telemetry: &Telemetry,
    client: &FirecrackerClient,
    tenant_id: &str,
    template: &VmTemplate,
    net: Option<&NetAttach>,
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
    // When a network attachment is present the guest IP/gateway travel on
    // the kernel command line so /init configures the address the host
    // allocated (no hardcoded-IP mismatch).
    let boot_cfg = match net {
        Some(n) => boot_source_with_net(
            &template.kernel_path,
            &template.initrd_path,
            &n.vm_ip,
            &n.gateway,
            tenant_id,
        ),
        None => boot_source(&template.kernel_path, &template.initrd_path),
    };
    let net_cfg = net.map(|n| network_interface(&n.tap_name, &n.guest_mac));
    // Only attach a root drive when a rootfs path is configured; initramfs-only
    // VMs boot from the initrd and need no block device.
    let root = if template.rootfs_path.trim().is_empty() {
        None
    } else {
        Some(root_drive(&template.rootfs_path))
    };

    let result = boot_vm_inner(client, &machine, &boot_cfg, net_cfg.as_ref(), root.as_ref()).await;

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
    net: Option<&crate::firecracker::NetworkInterfaceConfig>,
    root: Option<&crate::firecracker::config::DriveConfig>,
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
    // Attach the virtual NIC BEFORE InstanceStart — after start the network
    // config is frozen and the guest would have no eth0.
    if let Some(net) = net {
        let net_body =
            serde_json::to_value(net).map_err(|e| CoreError::Vm(e.to_string()))?;
        let net_path = format!("/network-interfaces/{}", net.iface_id);
        client
            .put(&net_path, &net_body)
            .await
            .map_err(|e| CoreError::Vm(format!("network interface: {e}")))?;
    }
    // Attach a balloon device (stats only: amount 0 reclaims nothing).
    // Gives per-VM memory pressure via GET /balloon/stats for the dashboard.
    // Best-effort: old kernels without virtio_balloon ignore it; a failure
    // here must never fail the boot.
    let balloon_body = serde_json::json!({
        "amount_mb": 0,
        "deflate_on_oom": true,
        "stats_polling_interval_s": 60,
    });
    // Intentionally ignored: kernels without virtio_balloon work fine,
    // the dashboard just shows allocated memory instead of live pressure.
    let _ = client.put("/balloon", &balloon_body).await;
    // Attach a root drive only when one is configured (initramfs-only VMs
    // boot straight from the initrd and need no block device).
    if let Some(root) = root {
        let drive_body =
            serde_json::to_value(root).map_err(|e| CoreError::Vm(e.to_string()))?;
        let drive_path = format!("/drives/{}", root.drive_id);
        client
            .put(&drive_path, &drive_body)
            .await
            .map_err(|e| CoreError::Vm(e.to_string()))?;
    }
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
