//! Firewall rule helpers.

use shared::{FirewallAction, FirewallRule};

/// Build a deny rule restricting a VM's traffic.
pub fn deny_all(src: &str) -> FirewallRule {
    FirewallRule {
        id: format!("fw_{src}"),
        action: FirewallAction::Deny,
        protocol: "any".to_string(),
        src: Some(src.to_string()),
        dst: None,
        port: None,
    }
}

/// Build an allow rule for a specific dst/port.
pub fn allow_dst(src: &str, dst: &str, port: u16) -> FirewallRule {
    FirewallRule {
        id: format!("fw_{src}_{dst}_{port}"),
        action: FirewallAction::Allow,
        protocol: "tcp".to_string(),
        src: Some(src.to_string()),
        dst: Some(dst.to_string()),
        port: Some(port),
    }
}
