//! Shared domain types for the wrapper system.

pub mod category;
pub mod deployment;
pub mod events;
pub mod network;
pub mod session;
pub mod tenant;
pub mod vm;

pub use category::{Category, CategoryStatus};
pub use deployment::{ArtifactType, DeploymentArtifact, DeploymentConfig, DeploymentManifest};
pub use events::{EventCategory, EventType};
pub use network::{
    FirewallAction, FirewallRule, IpAllocation, NetworkConfig, TapDevice, VsockConfig,
};
pub use session::{AuthToken, Session};
pub use tenant::{Tenant, TenantStatus};
pub use vm::{VmInfo, VmState, VmTemplate, VmTemplateStatus};

/// Convenience alias for a map used to pass arbitrary structured context.
pub type DataMap = std::collections::HashMap<String, serde_json::Value>;