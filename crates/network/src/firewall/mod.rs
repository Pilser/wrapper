//! Per-VM firewall rule management.

pub mod enforce;
pub mod rules;

use std::sync::Mutex;

use shared::FirewallRule;
use telemetry::{Level, Telemetry};

/// Tracks firewall rules per VM.
#[derive(Default)]
pub struct FirewallManager {
    rules: Mutex<Vec<FirewallRule>>,
}

impl Clone for FirewallManager {
    fn clone(&self) -> Self {
        Self {
            rules: Mutex::new(self.rules.lock().unwrap().clone()),
        }
    }
}

impl FirewallManager {
    pub fn with_telemetry(_telemetry: Telemetry) -> Self {
        Self { rules: Mutex::new(Vec::new()) }
    }

    pub fn add_rule(&self, rule: FirewallRule, telemetry: &Telemetry) -> anyhow::Result<()> {
        self.rules.lock().unwrap().push(rule.clone());
        telemetry.builder().level(Level::Info).kind("network.firewall.rule_added")
            .category("network").message(format!("rule {} added", rule.id)).emit();
        Ok(())
    }

    pub fn remove_rule(&self, id: &str, telemetry: &Telemetry) -> anyhow::Result<()> {
        self.rules.lock().unwrap().retain(|r| r.id != id);
        telemetry.builder().level(Level::Info).kind("network.firewall.rule_removed")
            .category("network").message(format!("rule {id} removed")).emit();
        Ok(())
    }

    pub fn rules(&self) -> Vec<FirewallRule> {
        self.rules.lock().unwrap().clone()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.lock().unwrap().is_empty()
    }
}
