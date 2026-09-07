//! Drive helpers for Firecracker.

use super::config::DriveConfig;

/// Build the rootfs drive for a boot.
pub fn root_drive(path: &str) -> DriveConfig {
    DriveConfig {
        drive_id: "rootfs".to_string(),
        path_on_host: path.to_string(),
        is_read_only: true,
        is_root_device: true,
    }
}

/// Build the tenant data disk drive.
pub fn data_drive(path: &str) -> DriveConfig {
    DriveConfig {
        drive_id: "data_disk".to_string(),
        path_on_host: path.to_string(),
        is_read_only: false,
        is_root_device: false,
    }
}
