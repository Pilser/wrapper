//! Tenant — the entity that owns a private VM and its per-tenant database.

use serde::{Deserialize, Serialize};

/// Lifecycle state of a tenant (and by extension its VM).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TenantStatus {
    #[default]
    Active,
    Suspended,
    Booting,
    Ready,
    Asleep,
    Deleted,
}

impl TenantStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TenantStatus::Active => "active",
            TenantStatus::Suspended => "suspended",
            TenantStatus::Booting => "booting",
            TenantStatus::Ready => "ready",
            TenantStatus::Asleep => "asleep",
            TenantStatus::Deleted => "deleted",
        }
    }
}

/// A tenant record mirrors one row in the global `tenants` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub category_id: String,
    pub vm_ip: Option<String>,
    pub vm_process_id: Option<i64>,
    pub vm_template_id: Option<String>,
    pub tap_device: Option<String>,
    pub status: TenantStatus,
    pub db_path: String,
    pub created_at: String,
    pub last_login: Option<String>,
    pub category_changed_at: Option<String>,
}