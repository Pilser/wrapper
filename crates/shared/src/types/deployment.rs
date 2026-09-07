//! Deployment configuration and category-based packages.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    SupervisorPlugin,
    AgentPlugin,
    Config,
    Binary,
    Command,
}

/// One artifact the wrapper pushes to a VM as part of a deployment package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentArtifact {
    pub artifact_type: ArtifactType,
    pub name: String,
    pub path: String,
    pub checksum: String,
    #[serde(default)]
    pub run_after_install: bool,
}

/// A package sent to a supervisor for the tenant's current category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentManifest {
    pub tenant_id: String,
    pub category: String,
    pub artifacts: Vec<DeploymentArtifact>,
}

/// A row in the global `deployment_configs` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub id: String,
    pub category_id: String,
    pub config_type: String,
    pub name: String,
    pub path: String,
    pub content: Option<Vec<u8>>,
    pub checksum: Option<String>,
    pub priority: i32,
    pub created_at: String,
    pub updated_at: String,
}