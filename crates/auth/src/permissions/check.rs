//! Permission checks.

use super::roles::Role;

pub fn check_permission(role: Role, resource: &str, action: &str) -> bool {
    let _ = (resource, action);
    match role {
        Role::Admin => true,
        Role::Staff => true,
        Role::ReadOnly => action == "read",
        Role::Student => action == "read",
    }
}
