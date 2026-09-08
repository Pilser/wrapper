//! VM lifecycle types.

use serde::{Deserialize, Serialize};

/// The runtime state of a single microVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VmState {
    #[default]
    Unknown,
    Booting,
    Running,
    ShuttingDown,
    Suspended,
    Snapshotting,
    Restoring,
    Failed,
    Destroyed,
}

impl VmState {
    pub fn is_ready(&self) -> bool {
        matches!(self, VmState::Running)
    }
}

/// A virtual machine template (rootfs + kernel) shared by all VMs of a category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmTemplate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub rootfs_path: String,
    pub kernel_path: String,
    /// Path to the initramfs (cpio.gz) used as the guest root filesystem.
    pub initrd_path: String,
    pub vcpu_count: u32,
    pub memory_mb: u32,
    pub requires_gpu: bool,
    pub status: VmTemplateStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VmTemplateStatus {
    #[default]
    Active,
    Retired,
}

/// Runtime info about an individual VM instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmInfo {
    pub id: String,
    pub tenant_id: String,
    pub template_id: String,
    pub state: VmState,
    pub ip: Option<String>,
    pub tap_device: Option<String>,
    pub vcpu: u32,
    pub memory_mb: u32,
}
impl VmTemplateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            VmTemplateStatus::Active => "active",
            VmTemplateStatus::Retired => "retired",
        }
    }
}
