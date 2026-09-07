//! Action taken when a VM is idle too long.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdleAction {
    None,
    Shutdown,
    Snapshot,
    Notify,
}

impl IdleAction {
    /// Choose an action based on how many seconds the VM has been idle.
    pub fn from_secs(idle_secs: u64) -> Self {
        match idle_secs {
            0..=1800 => IdleAction::None,
            1801..=3600 => IdleAction::Notify,
            3601..=7200 => IdleAction::Snapshot,
            _ => IdleAction::Shutdown,
        }
    }
}
