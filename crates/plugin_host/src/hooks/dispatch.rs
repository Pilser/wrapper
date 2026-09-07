//! The hook dispatcher: fan a hook event out to subscribed handlers.

use std::sync::Mutex;

use serde_json::Value;

use super::error::HookError;
use core::HookPoint;

pub type HookCallback = Arc<dyn Fn(&HookPoint, &Value) -> std::result::Result<(), HookError> + Send + Sync>;

use std::sync::Arc;

/// A registered hook subscription.
#[derive(Clone)]
pub struct HookSubscription {
    pub id: String,
    pub point: HookPoint,
    pub callback: HookCallback,
}

/// Dispatches hook events to subscribers, filtered and ordered.
#[derive(Default)]
pub struct HookDispatcher {
    subs: Mutex<Vec<HookSubscription>>,
}

impl HookDispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, point: HookPoint, callback: HookCallback) -> String {
        let id = format!("hook_{}", uuid::Uuid::new_v4().simple());
        self.subs.lock().unwrap().push(HookSubscription { id: id.clone(), point, callback });
        id
    }

    pub fn unregister(&self, id: &str) {
        self.subs.lock().unwrap().retain(|s| s.id != id);
    }

    pub fn dispatch(&self, point: HookPoint, event: &Value) {
        let active: Vec<HookSubscription> = self
            .subs
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.point == point)
            .filter(|s| s.accepts(event))
            .cloned()
            .collect();
        for sub in active {
            if let Err(e) = (sub.callback)(&point, event) {
                eprintln!("hook {} failed: {e}", sub.id);
            }
        }
    }
}
