//! List tap devices from `/proc/net/dev`.

use core::TapDevice;

pub fn list_taps() -> anyhow::Result<Vec<TapDevice>> {
    let content = std::fs::read_to_string("/proc/net/dev")?;
    let mut out = Vec::new();
    for line in content.lines().skip(2) {
        let name = line.split(':').next().unwrap_or("").trim().to_string();
        if name.starts_with("tap") || name.starts_with("vmtap") {
            out.push(TapDevice { name, ip: String::new() });
        }
    }
    Ok(out)
}
