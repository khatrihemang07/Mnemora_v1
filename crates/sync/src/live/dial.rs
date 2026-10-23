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
        let (won, accepted) = tokio::join!(
            race(&mine, vec![silent_addr, refused, good], each),
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
        let won = race(&mine, addrs, Duration::from_millis(300)).await;
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
