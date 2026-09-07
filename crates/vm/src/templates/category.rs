//! Category <-> template mapping.

use std::collections::HashMap;

#[derive(Default)]
pub struct CategoryTemplateMap {
    map: HashMap<String, String>,
}

impl CategoryTemplateMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn assign(&mut self, category_id: impl Into<String>, template_id: impl Into<String>) {
        self.map.insert(category_id.into(), template_id.into());
    }

    pub fn template_for(&self, category_id: &str) -> Option<&str> {
        self.map.get(category_id).map(String::as_str)
    }
}
