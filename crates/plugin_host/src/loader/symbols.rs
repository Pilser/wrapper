//! Symbol resolution through the plugin ABI.

use shared::PluginInfo;
use libloading::Library;

/// Required exported symbols for every plugin.
pub const REQUIRED_SYMBOLS: [&str; 3] = ["plugin_info", "register", "unregister"];

/// SAFETY contract: caller must only pass libraries whose symbols match the
/// plugin ABI (see REQUIRED_SYMBOLS).
///
/// Resolve `plugin_info() -> shared::PluginInfo`.
pub unsafe fn plugin_info(lib: &Library) -> anyhow::Result<PluginInfo> {
    // SAFETY: the library stream is only used inside this function; callers
    // must have validated the ABI via the loader.
    let f: libloading::Symbol<unsafe extern "C" fn() -> PluginInfo> =
        unsafe { lib.get(b"plugin_info\0")? };
    // SAFETY: calling the plugin entry point is inherently unsafe but is the
    // core contract of the plugin system.
    Ok(unsafe { f() })
}
