//! What a plugin is allowed to do.

#[derive(Debug, Clone, Copy)]
pub struct PluginPermissions {
    pub read_db: bool,
    pub write_db: bool,
    pub control_vm: bool,
    pub network: bool,
}

impl Default for PluginPermissions {
    fn default() -> Self {
        Self { read_db: false, write_db: false, control_vm: false, network: false }
    }
}

impl PluginPermissions {
    pub fn trusted() -> Self {
        Self { read_db: true, write_db: true, control_vm: true, network: true }
    }
}
