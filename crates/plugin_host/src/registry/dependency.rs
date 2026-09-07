//! Simple dependency ordering of plugin load.

use std::collections::{HashMap, HashSet};

/// Order plugin names so that declared dependencies load first.
pub fn order_by_dependencies(
    deps: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let mut ordered = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut visit: Vec<String> = deps.keys().cloned().collect();
    while let Some(name) = { visit.pop() } {
        if !seen.insert(name.clone()) {
            continue;
        }
        if let Some(ds) = deps.get(&name) {
            for d in ds {
                if !seen.contains(d) && deps.contains_key(d) {
                    visit.push(d.clone());
                }
            }
        }
        ordered.push(name);
    }
    ordered
}
