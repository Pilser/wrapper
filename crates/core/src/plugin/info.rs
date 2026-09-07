//! Static metadata every plugin exposes through `plugin_info()`.

use serde::{Deserialize, Serialize};

use super::HookPoint;

/// Category of plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PluginType {
    #[default]
    Builtin,
    External,
    Experimental,
}

/// Semantic version of a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PluginVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl PluginVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl std::fmt::Display for PluginVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Metadata describing a loaded plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: PluginVersion,
    pub description: String,
    pub plugin_type: PluginType,
    pub author: String,
    /// The hook points this plugin subscribes to.
    pub hooks: Vec<HookPoint>,
}

impl PluginInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: PluginVersion::default(),
            description: String::new(),
            plugin_type: PluginType::Builtin,
            author: String::new(),
            hooks: Vec::new(),
        }
    }
}