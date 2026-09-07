//! State-change payload used for `state.changed` events.

use serde::{Serialize, Deserialize};

/// A state transition (e.g. VM booting -> running).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatePayload {
    pub entity: String,
    pub entity_id: String,
    pub from: String,
    pub to: String,
    pub reason: Option<String>,
}