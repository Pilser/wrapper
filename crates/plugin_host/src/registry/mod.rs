//! Registry of loaded plugins.

pub mod dependency;
pub mod lookup;
pub mod priority;
pub mod store;

pub use store::PluginRegistry;
