//! Generate an audit report.

use super::log::AuditLog;

pub fn report(log: &AuditLog) -> String {
    log.entries().join("\n")
}
