//! Reaching a device: every address we know for it is tried at once, each for a few seconds, and
//! the first connection that passes the handshake wins. A stale address therefore never delays
//! the good one.
//!
//! Nothing here waits for discovery: a remembered or typed address is dialled whether or not mDNS
//! found anything, which is what keeps a Mac that macOS keeps off the local network reachable (it
//! still accepts connections, and the phone dials it). Tailscale addresses (outside the Local
//! Network permission) are started before the LAN ones.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinSet;

use crate::addresses;
use crate::local_network::LocalNetwork;
use crate::net::{Established, NetConfig, dial};

/// The addresses to try for a device: Tailscale ones first, then what discovery just found, then
/// what worked before (the most recent first), then an address the person typed. Each only once.
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
    // Stable: the order inside each group stays.
    out.sort_by_key(|addr| !addresses::is_tailscale(addr.ip()));
    out
}

/// Dials every address in parallel and returns the first connection that is established. What
/// the dials to LAN addresses show is told to `local`, which decides whether the system keeps
/// this device off the local network.
pub(super) async fn race(
    config: &NetConfig,
    addrs: Vec<SocketAddr>,
    each: Duration,
    local: &Arc<LocalNetwork>,
) -> Option<Established> {
    let mut attempts = JoinSet::new();
    for addr in addrs {
        let (config, local) = (config.clone(), Arc::clone(local));
        attempts.spawn(async move {
            let result = tokio::time::timeout(each, dial(&config, addr)).await;
            match &result {
                Ok(Ok(_)) => local.dial_worked(addr),
                Ok(Err(e)) => local.dial_failed(addr, e),
                Err(_) => {}
            }
            result
        });
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
    fn tailscale_addresses_come_first_then_found_remembered_and_typed_without_repeats() {
        let found = [a("192.168.1.9:47311")];
        let remembered = [a("100.64.0.2:47311"), a("192.168.1.9:47311")];
        let typed = [a("100.64.0.7:47311"), a("10.0.0.4:47311")];
        assert_eq!(
            candidates(&found, &remembered, &typed),
            [
                a("100.64.0.2:47311"),
                a("100.64.0.7:47311"),
                a("192.168.1.9:47311"),
                a("10.0.0.4:47311"),
            ]
        );
    }

    #[test]
    fn a_tailscale_address_in_ipv6_is_early_too() {
        let found = [a("192.168.1.9:47311")];
        let remembered = [a("[fd7a:115c:a1e0::7]:47311")];
        assert_eq!(
            candidates(&found, &remembered, &[]),
            [a("[fd7a:115c:a1e0::7]:47311"), a("192.168.1.9:47311")]
        );
    }

    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;
    use std::time::Instant;

    use mnemora_core::db::{self, migrate};
    use mnemora_core::env::Environment;
    use mnemora_core::hlc;
    use mnemora_core::module::CoreModule;

    use crate::SyncModule;
    use crate::net::{DeviceIdentity, Handshake, Incoming, Listener, LocalHandshake, Trust};

    fn config(name: &'static str) -> NetConfig {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        migrate::run(&mut conn, &[&CoreModule, &SyncModule], dir.path()).unwrap();
        hlc::set_device_id(&conn, name).unwrap();
        let local = Handshake::local(&conn, Environment::Production, name, None, vec![]).unwrap();
        NetConfig {
            identity: Arc::new(DeviceIdentity::generate().unwrap()),
            trust: Trust::new(),
            local: LocalHandshake::fixed(local),
        }
    }

    const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    #[tokio::test]
    async fn a_stale_or_silent_address_does_not_delay_the_good_one() {
        let (mine, theirs) = (config("mine"), config("theirs"));
        mine.trust.pin(theirs.identity.public_key(), "theirs");
        theirs.trust.pin(mine.identity.public_key(), "mine");
        let mut listener = Listener::bind(theirs.clone(), LOCALHOST, 0).await.unwrap();
        let good = listener.local_addr();
        // Refused at once, and accepted by a socket that never answers the handshake.
        let refused = std::net::TcpListener::bind((LOCALHOST, 0)).unwrap();
        let refused = refused.local_addr().unwrap();
        let silent = std::net::TcpListener::bind((LOCALHOST, 0)).unwrap();
        let (silent_addr, _hold) = (silent.local_addr().unwrap(), &silent);
        let each = Duration::from_secs(3);

        let started = Instant::now();
        let local = LocalNetwork::off();
        let (won, accepted) = tokio::join!(
            race(&mine, vec![silent_addr, refused, good], each, &local),
            listener.accept()
        );
        assert!(won.is_some(), "the good address connects");
        assert!(matches!(accepted, Some(Incoming::Peer(_))));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn every_address_is_given_only_its_own_few_seconds() {
        let mine = config("mine");
        let silent = std::net::TcpListener::bind((LOCALHOST, 0)).unwrap();
        let addrs = vec![silent.local_addr().unwrap(); 1];
        let started = Instant::now();
        let won = race(
            &mine,
            addrs,
            Duration::from_millis(300),
            &LocalNetwork::off(),
        )
        .await;
        assert!(won.is_none());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn nothing_known_means_nothing_to_try() {
        assert!(candidates(&[], &[], &[]).is_empty());
    }
}
