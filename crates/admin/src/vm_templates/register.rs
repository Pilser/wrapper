//! Register a VM template (row in the global `vm_templates` table).

use core::{now_rfc3339, Error, Result, VmTemplate, VmTemplateStatus};
use telemetry::{Level, Telemetry};

use super::validate::validate_template;

/// Input for [`register_template`].
#[derive(Debug, Clone)]
pub struct RegisterTemplate {
    pub name: String,
    pub description: Option<String>,
    pub rootfs_path: String,
    pub kernel_path: String,
    pub vcpu_count: u32,
    pub memory_mb: u32,
    pub requires_gpu: bool,
}

/// Validate then insert via `db::global::vm_templates::insert`.
pub async fn register_template(
    db: &db::GlobalDb,
    telemetry: &Telemetry,
    input: RegisterTemplate,
) -> Result<VmTemplate> {
    if input.name.trim().is_empty() {
        return Err(Error::InvalidInput(
            "template name must not be empty".into(),
        ));
    }
    let now = now_rfc3339();
    let template = VmTemplate {
        id: format!("tpl_{}", uuid::Uuid::new_v4()),
        name: input.name,
        description: input.description,
        rootfs_path: input.rootfs_path,
        kernel_path: input.kernel_path,
        vcpu_count: input.vcpu_count,
        memory_mb: input.memory_mb,
        requires_gpu: input.requires_gpu,
        status: VmTemplateStatus::Active,
        created_at: now.clone(),
        updated_at: now,
    };
    validate_template(&template)?;
    let conn = db.conn().map_err(|e| Error::Database(e.to_string()))?;
    db::global::vm_templates::insert(&conn, &template)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;
    telemetry
        .builder()
        .level(Level::Info)
        .kind("vm.template.registered")
        .category("vm")
        .source("admin::vm_templates::register")
        .message(format!("VM template {} registered", template.id))
        .data("template_id", template.id.clone())
        .tag("admin")
        .tag("vm")
        .emit();
    Ok(template)
}
