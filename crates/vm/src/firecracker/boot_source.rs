//! Boot source configuration helper.

use super::config::BootSourceConfig;

pub fn boot_source(kernel_path: &str) -> BootSourceConfig {
    BootSourceConfig {
        kernel_image_path: kernel_path.to_string(),
        boot_args: Some("console=ttyS0 reboot=k panic=1 pci=off".to_string()),
    }
}
