//! Destroy a bridge.

pub(crate) fn run_ip(args: &[&str]) -> anyhow::Result<()> {
    let status = std::process::Command::new("ip").args(args).status()?;
    if !status.success() {
        anyhow::bail!("ip {:?} failed ({status})", args);
    }
    Ok(())
}

pub fn destroy(bridge: &str) -> anyhow::Result<()> {
    run_ip(&["link", "del", "dev", bridge])
}
