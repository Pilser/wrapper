//! Hot reload: reload every loaded plugin.

use crate::PluginHost;

pub fn reload_plugins(host: &PluginHost) -> anyhow::Result<()> {
    let names = host.list();
    for name in names {
        host.reload(&name)?;
    }
    Ok(())
}
