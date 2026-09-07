//! Category-based template routing for VM boots.

use std::collections::HashMap;

#[derive(Default)]
pub struct CategoryRouter {
    templates: HashMap<String, String>,
}

impl CategoryRouter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn map(&mut self, category_id: impl Into<String>, template_id: impl Into<String>) {
        self.templates.insert(category_id.into(), template_id.into());
    }

    pub fn template_for(&self, category_id: &str) -> Option<&str> {
        self.templates.get(category_id).map(String::as_str)
    }
}
