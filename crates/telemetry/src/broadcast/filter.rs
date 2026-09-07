//! Client-side subscription filters for the live event stream.

use crate::models::Event;

/// Describes which events a dashboard wants (subscription message).
#[derive(Debug, Clone, Default)]
pub struct BroadcastFilter {
    /// `vm.*`, `auth.login.*` style prefixes; empty = allow all.
    pub events: Vec<String>,
    /// levels to forward; empty = all.
    pub levels: Vec<String>,
    /// tenant ids; empty = all tenants.
    pub tenants: Vec<String>,
}

impl BroadcastFilter {
    /// True when this filter matches the event.
    pub fn matches(&self, ev: &Event) -> bool {
        if !self.levels.is_empty() {
            let lvl = ev.level.as_str();
            if !self.levels.iter().any(|l| l == lvl) {
                return false;
            }
        }
        if !self.tenants.is_empty() {
            let tid = ev.tenant_id.as_deref().unwrap_or_default();
            if !self.tenants.iter().any(|t| t == tid) {
                return false;
            }
        }
        if self.events.is_empty() {
            return true;
        }
        self.events
            .iter()
            .any(|pat| pattern_match(pat, &ev.kind))
    }
}

/// Match a `vm.*` style wildcard pattern against a dotted event kind.
fn pattern_match(pattern: &str, kind: &str) -> bool {
    if !pattern.ends_with('*') {
        return pattern == kind;
    }
    let prefix = &pattern[..pattern.len() - 1];
    kind.starts_with(prefix)
}