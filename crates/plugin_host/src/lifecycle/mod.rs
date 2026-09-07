//! Plugin host lifecycle management.

pub mod init;
pub mod reload;
pub mod shutdown;

pub use init::init_host;
pub use reload::reload_plugins;
pub use shutdown::shutdown_host;

use crate::PluginHost;

/// Run the standard lifecycle against a host.
pub fn standard_lifecycle(host: &PluginHost) -> anyhow::Result<()> {
    init_host(host)?;
    let _ = reload_plugins(host);
    Ok(())
}
