use std::net::{IpAddr, Ipv4Addr};

/// Detect the machine's LAN IP address, skipping TUN/VPN interfaces.
pub fn detect_lan_ip() -> IpAddr {
    const TUN_KEYWORDS: &[&str] = &[
        "tun", "tap", "wireguard", "tailscale", "vpn", "tunnel", "loopback",
    ];

    if let Ok(ifaces) = local_ip_address::list_afinet_netifas() {
        for (_name, ip) in &ifaces {
            let IpAddr::V4(v4) = *ip else { continue };
            if v4.is_loopback() || v4.is_link_local() {
                continue;
            }
            let name_lower = _name.to_lowercase();
            if TUN_KEYWORDS.iter().any(|kw| name_lower.contains(kw)) {
                continue;
            }
            return IpAddr::V4(v4);
        }
    }

    local_ip_address::local_ip().unwrap_or(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
}
