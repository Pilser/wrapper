//! Tenants (rows in the global `tenants` table).

pub mod bulk;
pub mod change_category;
pub mod create;
pub mod delete;
pub mod suspend;
pub mod update;

pub use bulk::bulk_update_category;
pub use change_category::{change_category, ChangeCategory};
pub use create::{create_tenant, CreateTenant};
pub use delete::delete_tenant;
pub use suspend::suspend_tenant;
pub use update::{update_tenant, UpdateTenant};
