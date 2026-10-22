//! Which of this device's addresses may be shown and shared. Only private IPv4, Tailscale's
//! 100.64.0.0/10 and its `fd7a:115c:a1e0::/48`; never loopback, link-local or public ones, so a
//! public address never ends up in anyone's data.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Whether `ip` may be shown to the user and gossiped to paired devices.
pub fn shareable(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || is_tailscale_v4(v4),
        IpAddr::V6(v6) => is_tailscale_v6(v6),
    }
}

fn is_tailscale_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    a == 100 && (64..=127).contains(&b)
}

fn is_tailscale_v6(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    s[0] == 0xfd7a && s[1] == 0x115c && s[2] == 0xa1e0
}

/// Whether `ip` is a Tailscale address.
pub fn is_tailscale(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_tailscale_v4(v4),
        IpAddr::V6(v6) => is_tailscale_v6(v6),
    }
}

/// This device's shareable addresses, IPv4 first, Tailscale last.
pub fn local() -> Vec<IpAddr> {
    let mut found: Vec<IpAddr> = if_addrs::get_if_addrs()
        .map(|all| {
            all.into_iter()
                .map(|i| i.ip())
                .filter(|ip| shareable(*ip))
                .collect()
        })
        .unwrap_or_default();
    found.sort_by_key(|ip| (is_tailscale(*ip), ip.is_ipv6(), *ip));
    found.dedup();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn private_and_tailscale_addresses_are_shareable() {
        for ok in [
            "192.168.1.20",
            "10.0.0.7",
            "172.16.5.5",
            "172.31.255.1",
            "100.64.0.1",
            "100.101.4.7",
            "100.127.255.254",
            "fd7a:115c:a1e0::1",
            "fd7a:115c:a1e0:ab12::7",
        ] {
            assert!(shareable(ip(ok)), "{ok}");
        }
    }

    #[test]
    fn loopback_link_local_and_public_addresses_are_not() {
        for bad in [
            "127.0.0.1",
            "169.254.1.1",
            "8.8.8.8",
            "100.63.255.255",
            "100.128.0.1",
            "172.32.0.1",
            "::1",
            "fe80::1",
            "2001:db8::1",
            "fd7a:115c:a1e1::1",
            "fd00::1",
        ] {
            assert!(!shareable(ip(bad)), "{bad}");
        }
    }
}
