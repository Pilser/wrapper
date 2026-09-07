//! Staff accounts, permissions, audit trail and oversight reports.

pub mod audit;
pub mod create;
pub mod oversight;
pub mod permissions;

pub use audit::{log_audit, AuditEntry};
pub use create::{create_staff, StaffMember};
pub use oversight::generate_oversight_report;
pub use permissions::{grant_permission, revoke_permission};
