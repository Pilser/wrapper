//! Unload a plugin (drop its library).

use telemetry::{Level, Telemetry};

use super::load::LoadOutcome;

pub fn unload_plugin(plugin: &LoadOutcome, telemetry: &Telemetry) -> anyhow::Result<()> {
    let name = plugin.name.clone();
    // Dropping `lib` closes the shared object.
    telemetry.builder().level(Level::Info).kind("plugin.unloaded")
        .category("plugin").message(format!("plugin {name} unloaded")).emit();
    Ok(())
}
