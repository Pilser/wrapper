//! Validate a VM template before registration.

use shared::VmTemplate;

pub fn validate(t: &VmTemplate) -> anyhow::Result<()> {
    if t.id.is_empty() {
        anyhow::bail!("template id is empty");
    }
    if t.vcpu_count == 0 {
        anyhow::bail!("template vcpu_count is zero");
    }
    if t.memory_mb == 0 {
        anyhow::bail!("template memory_mb is zero");
    }
    Ok(())
}
