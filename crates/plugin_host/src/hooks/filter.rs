//! Filters deciding which subscriptions fire for an event.

use serde_json::Value;

use super::dispatch::HookSubscription;

impl HookSubscription {
    /// Whether this subscription accepts the event payload.
    pub fn accepts(&self, _event: &Value) -> bool {
        // Plugins may implement finer filtering on their callback.
        true
    }
}

pub fn should_accept(_sub: &HookSubscription, _event: &Value) -> bool {
    true
}
