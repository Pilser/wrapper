//! Enforce the rule set on a tenant's tap device.

use core::FirewallAction;

use super::FirewallManager;

pub fn enforce(manager: &FirewallManager, tap: &str) -> anyhow::Result<(String, Vec<String>)> {
    let mut commands = Vec::new();
    for rule in manager.rules() {
        let action = match rule.action {
            FirewallAction::Allow => "-A",
            FirewallAction::Deny => "-D",
        };
        let mut args = format!("iptables {action} FORWARD -i {tap} -j DROP");
        if let Some(port) = rule.port {
            args.push_str(&format!(" -p {} --dport {port}", rule.protocol));
        }
        commands.push(args);
    }
    Ok(("enforced".to_string(), commands))
}
