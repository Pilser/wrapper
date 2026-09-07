//! Records which VM is running per tenant.

use std::collections::HashMap;
use std::sync::Mutex;

use shared::{VmInfo, VmState};

/// In-memory map tenant_id -> VM info.
#[derive(Clone, Default)]
pub struct VmRegistry {
    vms: std::sync::Arc<Mutex<HashMap<String, VmInfo>>>,
}

impl VmRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, info: VmInfo) {
        self.vms.lock().unwrap().insert(info.id.clone(), info);
    }

    pub fn get(&self, tenant_id: &str) -> Option<VmInfo> {
        self.vms.lock().unwrap().get(tenant_id).cloned()
    }

    pub fn set(&self, id: &str, state: &str) {
        if let Some(vm) = self.vms.lock().unwrap().get_mut(id) {
            vm.state = vm_state(state);
        }
    }

    pub fn remove(&self, id: &str) {
        self.vms.lock().unwrap().remove(id);
    }

    pub fn keys(&self) -> Vec<String> {
        self.vms.lock().unwrap().keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.vms.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.vms.lock().unwrap().is_empty()
    }
}

fn vm_state(s: &str) -> VmState {
    match s {
        "running" => VmState::Running,
        "booting" => VmState::Booting,
        "stopped" => VmState::Destroyed,
        "suspended" => VmState::Suspended,
        _ => VmState::Unknown,
    }
}
