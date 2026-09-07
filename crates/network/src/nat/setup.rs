//! Enable NAT on the host for the tenant network.

pub fn setup_nat(_iface: &str, _cidr: &str) -> anyhow::Result<Vec<String>> {
    let rules = vec![
        format!("iptables -t nat -A POSTROUTING -s {_cidr} -o {_iface} -j MASQUERADE"),
        "sysctl -w net.ipv4.ip_forward=1".to_string(),
    ];
    Ok(rules)
}
