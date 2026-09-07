//! NAT rule management.

pub fn add_masquerade(iface: &str, cidr: &str) -> anyhow::Result<()> {
    run_iptables(&["-t", "nat", "-A", "POSTROUTING", "-s", cidr, "-o", iface, "-j", "MASQUERADE"])
}

pub fn remove_masquerade(iface: &str, cidr: &str) -> anyhow::Result<()> {
    run_iptables(&["-t", "nat", "-D", "POSTROUTING", "-s", cidr, "-o", iface, "-j", "MASQUERADE"])
}

pub fn setup_forward(cidr: &str) -> anyhow::Result<()> {
    run_iptables(&["-A", "FORWARD", "-s", cidr, "-j", "ACCEPT"])
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
