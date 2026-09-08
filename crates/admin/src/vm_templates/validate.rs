//! Validate a VM template before registration or update.

use shared::{Error, Result, VmTemplate};

/// Check that paths are set and resources are sane.
pub fn validate_template(template: &VmTemplate) -> Result<()> {
    if template.name.trim().is_empty() {
        return Err(Error::InvalidInput(
            "template name must not be empty".into(),
        ));
    }
    if template.rootfs_path.trim().is_empty() {
        return Err(Error::InvalidInput(
            "rootfs_path must not be empty".into(),
        ));
    }
    if template.kernel_path.trim().is_empty() {
        return Err(Error::InvalidInput(
            "kernel_path must not be empty".into(),
        ));
    }
    if template.initrd_path.trim().is_empty() {
        return Err(Error::InvalidInput(
            "initrd_path must not be empty".into(),
        ));
    }
    if template.vcpu_count == 0 {
        return Err(Error::InvalidInput(
            "vcpu_count must be greater than zero".into(),
        ));
    }
    if template.memory_mb == 0 {
        return Err(Error::InvalidInput(
            "memory_mb must be greater than zero".into(),
        ));
    }
    Ok(())
}
