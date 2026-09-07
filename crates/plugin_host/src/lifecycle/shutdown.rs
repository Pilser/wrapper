//! Graceful shutdown: unload all plugins.

use crate::PluginHost;

pub fn shutdown_host(host: &PluginHost) -> anyhow::Result<()> {
    let names = host.list();
    for name in names {
        host.unload(&name)?;
    }
    Ok(())
}
