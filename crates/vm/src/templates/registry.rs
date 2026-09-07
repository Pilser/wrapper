//! Registry of known VM templates.

use std::collections::HashMap;
use std::sync::Mutex;

use shared::VmTemplate;

#[derive(Default)]
pub struct TemplateRegistry {
    templates: Mutex<HashMap<String, VmTemplate>>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, template: VmTemplate) {
        self.templates.lock().unwrap().insert(template.id.clone(), template);
    }

    pub fn get(&self, id: &str) -> Option<VmTemplate> {
        self.templates.lock().unwrap().get(id).cloned()
    }

    pub fn list(&self) -> Vec<VmTemplate> {
        self.templates.lock().unwrap().values().cloned().collect()
    }

    pub fn remove(&self, id: &str) -> Option<VmTemplate> {
        self.templates.lock().unwrap().remove(id)
    }
}
