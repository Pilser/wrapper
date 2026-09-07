//! Numerical priority ordering.

/// Order entries by their priority number (lowest first) while keeping the
/// original ordering for ties.
pub fn sort_by_priority(entries: &mut Vec<(String, i32)>) {
    entries.sort_by(|a, b| a.1.cmp(&b.1));
}
