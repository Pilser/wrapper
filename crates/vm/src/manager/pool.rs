//! Warm pool of pre-booted VMs.

use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Default)]
pub struct VmPool {
    idle: Mutex<VecDeque<String>>,
    max: usize,
}

impl VmPool {
    pub fn new(max: usize) -> Self {
        Self { idle: Mutex::new(VecDeque::new()), max }
    }

    pub fn push(&self, vm_id: String) -> bool {
        let mut q = self.idle.lock().unwrap();
        if q.len() >= self.max {
            return false;
        }
        q.push_back(vm_id);
        true
    }

    pub fn pop(&self) -> Option<String> {
        self.idle.lock().unwrap().pop_front()
    }

    pub fn len(&self) -> usize {
        self.idle.lock().unwrap().len()
    }
}
