//! Boot source configuration helper.

use super::config::BootSourceConfig;

/// Build a boot source for an initramfs-based VM.
pub fn boot_source(kernel_path: &str, initrd_path: &str) -> BootSourceConfig {
    BootSourceConfig {
        kernel_image_path: kernel_path.to_string(),
        initrd_path: Some(initrd_path.to_string()),
        // Boot args for an initramfs root: no quiet, no disk mounts needed.
        boot_args: Some("console=ttyS0 reboot=k panic=1 pci=off init=/init".to_string()),
    }
}

/// Build a boot source for an initramfs-based VM, passing the guest IP and
/// gateway on the kernel command line. The guest `/init` reads `vm_ip=` and
/// `gw=` from `/proc/cmdline` so the address the host allocated (IP pool) is
/// the address the guest configures — no more hardcoded-IP mismatch.
pub fn boot_source_with_net(
    kernel_path: &str,
    initrd_path: &str,
    vm_ip: &str,
    gateway: &str,
) -> BootSourceConfig {
    BootSourceConfig {
        kernel_image_path: kernel_path.to_string(),
        initrd_path: Some(initrd_path.to_string()),
        boot_args: Some(format!(
            "console=ttyS0 reboot=k panic=1 pci=off init=/init vm_ip={vm_ip} gw={gateway}"
        )),
    }
}
