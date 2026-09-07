//! Validate a plugin's metadata before registering it.

use shared::{PluginInfo, PluginType};

use super::symbols::REQUIRED_SYMBOLS;

pub fn validate(info: &PluginInfo) -> anyhow::Result<()> {
    if info.name.is_empty() {
        anyhow::bail!("plugin has an empty name");
    }
    let _ = PluginType::External;
    let _ = REQUIRED_SYMBOLS;
    Ok(())
}
