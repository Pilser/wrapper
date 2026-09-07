//! The `Plugin` trait every wrapper plugin implementation conforms to.

use super::{PluginContext, PluginInfo};
use crate::Result;

/// Runtime contract for a wrapper plugin.
///
/// A plugin is registered once at load, receives its [`PluginContext`], and is
/// notified at unload. All observable behaviour comes from the hooks the
/// plugin subscribes to in [`PluginInfo::hooks`].
pub trait Plugin: Send + Sync {
    /// Static metadata describing this plugin.
    fn info(&self) -> PluginInfo;

    /// Called once after the `.so`/plugin is loaded and resolved.
    fn register(&mut self, ctx: PluginContext) -> Result<()> {
        let _ = ctx;
        Ok(())
    }

    /// Called during shutdown or hot-reload before the plugin is dropped.
    fn unregister(&mut self, ctx: PluginContext) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}