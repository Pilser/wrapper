//! Validate a VM template before registration or update.

use shared::{Error, Result, VmTemplate};

/// Check that the name is set and resources are sane. Kernel/initrd/rootfs
/// paths are legacy Firecracker fields (kept in the row, unused by the
/// docker backend, never required).
pub fn validate_template(template: &VmTemplate) -> Result<()> {
    if template.name.trim().is_empty() {
        return Err(Error::InvalidInput(
            "template name must not be empty".into(),
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
