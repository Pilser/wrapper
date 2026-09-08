//! Firecracker machine/drive/boot-source configuration.

use serde::{Deserialize, Serialize};

/// Boot source (kernel + initrd) configuration for a microVM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootSourceConfig {
    pub kernel_image_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initrd_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boot_args: Option<String>,
}

/// A block device attached to a microVM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveConfig {
    pub drive_id: String,
    pub path_on_host: String,
    pub is_read_only: bool,
    pub is_root_device: bool,
}

/// Resource configuration for a microVM (`/machine-config`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineConfig {
    pub vcpu_count: u32,
    pub mem_size_mib: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_template: Option<String>,
}

impl MachineConfig {
    pub fn new(vcpu: u32, mem_mib: u32) -> Self {
        Self { vcpu_count: vcpu, mem_size_mib: mem_mib, cpu_template: None }
    }
}
