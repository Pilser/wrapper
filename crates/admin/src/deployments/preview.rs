//! Dry-run preview of a [`shared::DeploymentManifest`].

use shared::DeploymentManifest;

/// Human-readable summary of what a deployment would ship.
#[derive(Debug, Clone)]
pub struct DeploymentPreview {
    pub tenant_id: String,
    pub category: String,
    pub artifact_count: usize,
    pub summary: String,
}

/// Summarize a manifest without sending anything to a VM.
pub fn preview_deployment(manifest: &DeploymentManifest) -> DeploymentPreview {
    let artifact_count = manifest.artifacts.len();
    let summary = format!(
        "deployment for tenant {} (category {}): {artifact_count} artifact(s)",
        manifest.tenant_id, manifest.category
    );
    DeploymentPreview {
        tenant_id: manifest.tenant_id.clone(),
        category: manifest.category.clone(),
        artifact_count,
        summary,
    }
}
