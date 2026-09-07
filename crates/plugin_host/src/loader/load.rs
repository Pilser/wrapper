//! Load a `.so` plugin and resolve its metadata.

use std::path::Path;

use shared::PluginInfo;
use libloading::Library;
use telemetry::{Level, Telemetry};

use super::{symbols, validate};

/// Result of a successful load.
pub struct LoadOutcome {
    pub name: String,
    pub info: PluginInfo,
    pub lib: Library,
}

/// dlopen a plugin library and validate its ABI surface.
pub fn load_library(path: &Path, telemetry: Telemetry) -> anyhow::Result<LoadOutcome> {
    // SAFETY: the plugin loader is the one place that touches native symbols.
    unsafe {
        let lib = Library::new(path)?;
        let info = symbols::plugin_info(&lib)?;
        validate::validate(&info)?;
        let name = info.name.clone();
        telemetry.builder().level(Level::Info).kind("plugin.loaded")
            .category("plugin").message(format!("plugin {name} loaded")).emit();
        Ok(LoadOutcome { name, info, lib })
    }
}
