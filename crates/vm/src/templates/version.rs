//! Template versioning.

/// A versioned template reference.
#[derive(Debug, Clone, Default)]
pub struct TemplateVersion {
    pub template_id: String,
    pub version: String,
}

impl TemplateVersion {
    pub fn new(template_id: impl Into<String>, version: impl Into<String>) -> Self {
        Self { template_id: template_id.into(), version: version.into() }
    }
}
