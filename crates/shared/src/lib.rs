//! `core` — shared types, plugin contract and utilities for the wrapper.

#![forbid(unsafe_code)]

pub mod binaries;
pub mod error;
pub mod plugin;
pub mod types;
pub mod util;

pub use error::{Error, Result};
pub use plugin::{
    HookPoint, Plugin, PluginApi, PluginContext, PluginInfo, PluginType, PluginVersion,
};
pub use types::category::{Category, CategoryStatus};
pub use types::deployment::{ArtifactType, DeploymentArtifact, DeploymentConfig, DeploymentManifest};
pub use types::events::{EventCategory, EventType};
pub use types::network::{
    FirewallAction, FirewallRule, IpAllocation, NetworkConfig, TapDevice, VsockConfig,
};
pub use types::session::{AuthToken, Session};
pub use types::tenant::{Tenant, TenantStatus};
pub use types::vm::{VmInfo, VmState, VmTemplate, VmTemplateStatus};
pub use util::{now_rfc3339, now_unix_millis};
