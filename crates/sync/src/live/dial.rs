//! Reaching a device: every address we know for it is tried at once, each for a few seconds, and
//! the first connection that passes the handshake wins. A stale address therefore never delays
//! the good one.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::task::JoinSet;

use crate::net::{Established, NetConfig, dial};

/// The addresses to try for a device: what discovery just found, then what worked before (the
/// most recent first), then an address the person typed. Each only once.
pub(super) fn candidates(
    found: &[SocketAddr],
    remembered: &[SocketAddr],
    typed: &[SocketAddr],
) -> Vec<SocketAddr> {
    let mut out: Vec<SocketAddr> = Vec::new();
    for addr in found.iter().chain(remembered).chain(typed) {
        if !out.contains(addr) {
            out.push(*addr);
        }
    }
    out
}

/// Dials every address in parallel and returns the first connection that is established.
pub(super) async fn race(
    config: &NetConfig,
    addrs: Vec<SocketAddr>,
    each: Duration,
) -> Option<Established> {
    let mut attempts = JoinSet::new();
    for addr in addrs {
        let config = config.clone();
        attempts.spawn(async move { tokio::time::timeout(each, dial(&config, addr)).await });
    }
    while let Some(result) = attempts.join_next().await {
        match result {
            Ok(Ok(Ok(established))) => return Some(established),
            Ok(Ok(Err(e))) => tracing::debug!(target: "sync", error = %e, "dial failed"),
            Ok(Err(_)) => tracing::debug!(target: "sync", "dial timed out"),
            Err(_) => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn found_addresses_come_first_then_remembered_then_typed_without_repeats() {
        let found = [a("192.168.1.9:47311")];
        let remembered = [a("100.64.0.2:47311"), a("192.168.1.9:47311")];
        let typed = [a("100.64.0.7:47311")];
        assert_eq!(
            candidates(&found, &remembered, &typed),
            [
                a("192.168.1.9:47311"),
                a("100.64.0.2:47311"),
                a("100.64.0.7:47311")
            ]
        );
    }

    #[test]
    fn nothing_known_means_nothing_to_try() {
        assert!(candidates(&[], &[], &[]).is_empty());
    }
}
