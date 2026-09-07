//! Isolate plugin execution context (placeholder).

/// Best-effort isolation guard; extended at runtime with seccomp land.
pub fn isolate(_permissions: &super::permissions::PluginPermissions) -> anyhow::Result<()> {
    Ok(())
}
