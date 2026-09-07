//! Update a VM template (row in the global `vm_templates` table).

use shared::{Error, Result};
use telemetry::{Level, Telemetry};

/// Patch for [`update_template`]; `None` fields are left unchanged.
#[derive(Debug, Clone, Default)]
pub struct UpdateTemplate {
    pub description: Option<String>,
    pub vcpu_count: Option<u32>,
    pub memory_mb: Option<u32>,
}

/// Apply the patch via `db::global::vm_templates::update` and emit
/// `vm.template.updated`.
pub async fn update_template(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    id: &str,
    patch: UpdateTemplate,
) -> Result<()> {
    if id.trim().is_empty() {
        return Err(Error::InvalidInput(
            "template id must not be empty".into(),
        ));
    }
    if let Some(vcpu) = patch.vcpu_count
        && vcpu == 0
    {
        return Err(Error::InvalidInput(
            "vcpu_count must be greater than zero".into(),
        ));
    }
    if let Some(memory) = patch.memory_mb
        && memory == 0
    {
        return Err(Error::InvalidInput(
            "memory_mb must be greater than zero".into(),
        ));
    }
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    let mut template = db::global::vm_templates::list(&conn)
        .await
        .map_err(|e| Error::Database(e.to_string()))?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| Error::NotFound(format!("VM template {id}")))?;
    if let Some(description) = patch.description {
        template.description = Some(description);
    }
    if let Some(vcpu_count) = patch.vcpu_count {
        template.vcpu_count = vcpu_count;
    }
    if let Some(memory_mb) = patch.memory_mb {
        template.memory_mb = memory_mb;
    }
    template.updated_at = shared::now_rfc3339();
    super::validate::validate_template(&template)?;
    db::global::vm_templates::update(&conn, &template)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("vm.template.updated")
        .category("vm")
        .source("admin::vm_templates::update")
        .message(format!("VM template {id} updated"))
        .data("template_id", id.to_string())
        .tag("admin")
        .tag("vm")
        .emit();
    Ok(())
}
