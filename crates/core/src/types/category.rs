//! Categories — the domain groupings (robotics, painting, music, staff, ...).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CategoryStatus {
    #[default]
    Active,
    Inactive,
}

/// A category ties a VM template to its deployment configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub vm_template_id: String,
    pub max_tenants: Option<i64>,
    pub status: CategoryStatus,
    pub created_at: String,
    pub updated_at: String,
}
impl CategoryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CategoryStatus::Active => "active",
            CategoryStatus::Inactive => "inactive",
        }
    }
}
