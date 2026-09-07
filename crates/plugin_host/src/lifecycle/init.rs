//! Initialize the plugin host (scan + load).

use crate::PluginHost;

pub fn init_host(host: &PluginHost) -> anyhow::Result<()> {
    host.init()
}
