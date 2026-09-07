//! Tracks last-active time per VM.

use std::collections::HashMap;
use std::sync::Mutex;

use super::timeout::IdleTimeout;
use super::action::IdleAction;

#[derive(Default)]
pub struct IdleTracker {
    last_active: Mutex<HashMap<String, u64>>,
}

impl IdleTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_active(&self, vm_id: &str, now: u64) {
        self.last_active.lock().unwrap().insert(vm_id.to_string(), now);
    }

    pub fn is_idle(&self, vm_id: &str, timeout: &IdleTimeout, now: u64) -> bool {
        let last = self.last_active.lock().unwrap().get(vm_id).copied().unwrap_or(now);
        now.saturating_sub(last) > timeout.as_secs()
    }

    pub fn remove(&self, vm_id: &str) {
        self.last_active.lock().unwrap().remove(vm_id);
    }

    pub fn prune_idle(&self, timeout: &IdleTimeout, now: u64) -> Vec<(String, IdleAction)> {
        let mut guard = self.last_active.lock().unwrap();
        let ids: Vec<String> = guard
            .iter()
            .filter(|(_, last)| now.saturating_sub(**last) > timeout.as_secs())
            .map(|(id, _)| id.clone())
            .collect();
        let mut pruned = Vec::new();
        for id in ids {
            if let Some(last) = guard.remove(&id) {
                pruned.push((id, IdleAction::from_secs(now.saturating_sub(last))));
            }
        }
        pruned
    }
}
