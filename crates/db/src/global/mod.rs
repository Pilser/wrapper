//! Global database (cross-tenant registry data).

pub mod categories;
pub mod category_assignments;
pub mod category_tools;
pub mod deployment_configs;
pub mod networks;
pub mod sessions;
pub mod staff;
pub mod store;
pub mod tenants;
pub mod vm_templates;

pub use category_assignments::CategoryAssignmentRow;
pub use staff::StaffRow;
pub use store::{GlobalDb, GlobalStore};
