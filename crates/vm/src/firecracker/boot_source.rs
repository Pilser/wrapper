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
