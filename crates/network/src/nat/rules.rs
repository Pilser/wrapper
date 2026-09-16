//! NAT rule management.

/// Add only when missing (`-C` check first): blind `-A` on every boot
/// piles hundreds of duplicate rules and burns host CPU on every packet.
fn ensure_iptables(args: &[&str]) -> anyhow::Result<()> {
    let mut check: Vec<&str> = args.to_vec();
    if let Some(pos) = check.iter().position(|&a| a == "-A") {
        check[pos] = "-C";
        if std::process::Command::new("iptables")
            .args(&check)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Ok(());
        }
    }
    run_iptables(args)
}

pub fn add_masquerade(iface: &str, cidr: &str) -> anyhow::Result<()> {
    ensure_iptables(&["-t", "nat", "-A", "POSTROUTING", "-s", cidr, "-o", iface, "-j", "MASQUERADE"])
}

pub fn remove_masquerade(iface: &str, cidr: &str) -> anyhow::Result<()> {
    run_iptables(&["-t", "nat", "-D", "POSTROUTING", "-s", cidr, "-o", iface, "-j", "MASQUERADE"])
}

pub fn setup_forward(cidr: &str) -> anyhow::Result<()> {
    ensure_iptables(&["-A", "FORWARD", "-s", cidr, "-j", "ACCEPT"])
}

fn run_iptables(args: &[&str]) -> anyhow::Result<()> {
    let status = std::process::Command::new("iptables").args(args).status()?;
    if !status.success() {
        anyhow::bail!("iptables {:?} failed ({status})", args);
    }
    Ok(())
}

/// Summarize the current NAT setup for a tenant network.
pub fn nat_rules(iface: &str, cidr: &str) -> Vec<String> {
    vec![
        format!("iptables -t nat -A POSTROUTING -s {cidr} -o {iface} -j MASQUERADE"),
        format!("iptables -A FORWARD -s {cidr} -j ACCEPT"),
    ]
}
