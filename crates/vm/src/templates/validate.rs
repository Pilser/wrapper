//! Validate a VM template before registration.

use core::VmTemplate;

pub fn validate(t: &VmTemplate) -> anyhow::Result<()> {
    if t.id.is_empty() {
        anyhow::bail!("template id is empty");
    }
    if t.rootfs_path.is_empty() {
        anyhow::bail!("template rootfs_path is empty");
    }
    if t.kernel_path.is_empty() {
        anyhow::bail!("template kernel_path is empty");
    }
    Ok(())
}
