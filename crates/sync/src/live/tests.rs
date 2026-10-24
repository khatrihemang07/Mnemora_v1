//! The session manager over real loopback sockets, between two in-memory devices with nothing to
//! sync: what is tested here is which connection survives and that a session runs over it.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use mnemora_core::db::{self, migrate};
use mnemora_core::env::Environment;
use mnemora_core::events::EventBus;
use mnemora_core::hlc;
use mnemora_core::module::CoreModule;
use rusqlite::Connection;
use tokio::sync::Notify;

use super::{Host, Live, Pace, Schedule};
use crate::discovery::Fixed;
use crate::local_network::{LocalNetwork, Rhythm};
use crate::net::{
    DeviceIdentity, Handshake, Incoming, Listener, LocalHandshake, NetConfig, PublicKey, Trust,
    dial,
};
use crate::pair::{Paired, PairingHost, PairingView, Stored};
use crate::{Registry, SyncError, SyncModule, Who};

const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

struct TestHost {
    device: String,
    conn: Mutex<Connection>,
    /// The next session this device starts panics (a bug in a connection task).
    fault: Arc<AtomicBool>,
    /// How often the Devices tab was told to read again.
    announced: Arc<AtomicUsize>,
}

impl PairingHost for TestHost {
    fn me(&self) -> Result<Who, SyncError> {
        Err(SyncError::NotReady)
    }

    fn handshake(&self) -> Result<Handshake, SyncError> {
        let conn = self.conn.lock().unwrap_or_else(PoisonError::into_inner);
        Handshake::local(&conn, Environment::Production, &self.device, None, vec![])
    }

    fn visible(&self) -> bool {
        false
    }

    fn pinned(&self) -> Vec<(PublicKey, String)> {
        Vec::new()
    }

    fn store(&self, _: &Paired) -> Result<Stored, SyncError> {
        Err(SyncError::NotReady)
    }

    fn show(&self, _: &PairingView) {}
}

impl Host for TestHost {
    fn with_db(
        &self,
        f: &mut dyn FnMut(&Connection) -> Result<(), SyncError>,
    ) -> Result<(), SyncError> {
        f(&self.conn.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn registry(&self) -> Registry {
        assert!(!self.fault.swap(false, Ordering::SeqCst), "test fault");
        Registry::new(Arc::new(Vec::new()))
    }

    fn bus(&self) -> EventBus {
        EventBus::new()
    }

    fn automatic(&self) -> bool {
        true
    }

    fn local_changes(&self) -> Arc<Notify> {
        Arc::default()
    }

    fn status_changed(&self) {
        self.announced.fetch_add(1, Ordering::SeqCst);
    }
}

struct Device {
    id: &'static str,
    config: NetConfig,
    listener: Listener,
    live: Live,
    fault: Arc<AtomicBool>,
    announced: Arc<AtomicUsize>,
}

impl Device {
    async fn new(id: &'static str) -> Self {
        Self::paced(id, Duration::from_secs(3)).await
    }

    async fn paced(id: &'static str, dial_each: Duration) -> Self {
        Self::build(id, dial_each, LocalNetwork::off()).await
    }

    async fn build(
        id: &'static str,
        dial_each: Duration,
        local_network: Arc<LocalNetwork>,
    ) -> Self {
        let mut conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        migrate::run(&mut conn, &[&CoreModule, &SyncModule], dir.path()).unwrap();
        hlc::set_device_id(&conn, id).unwrap();
        let fault = Arc::new(AtomicBool::new(false));
        let announced = Arc::new(AtomicUsize::new(0));
        let host = Arc::new(TestHost {
            device: id.to_owned(),
            conn: Mutex::new(conn),
            fault: Arc::clone(&fault),
            announced: Arc::clone(&announced),
        });
        let local = LocalHandshake::new({
            let host = Arc::clone(&host);
            move || host.handshake()
        });
        let config = NetConfig {
            identity: Arc::new(DeviceIdentity::generate().unwrap()),
            trust: Trust::new(),
            local,
        };
        let listener = Listener::bind(config.clone(), LOCALHOST, 0).await.unwrap();
        let pace = Pace {
            dial_each,
            // No retries during the test.
            retry: Schedule {
                steps: vec![Duration::from_secs(600)],
                steady: Duration::from_secs(600),
            },
            ..Pace::default()
        };
        let live = Live::start(
            host,
            config.clone(),
            Fixed::new(),
            pace,
            id.to_owned(),
            local_network,
            &tokio::runtime::Handle::current(),
        );
        Self {
            id,
            config,
            listener,
            live,
            fault,
            announced,
        }
    }

    fn addr(&self) -> SocketAddr {
        SocketAddr::new(LOCALHOST, self.listener.local_addr().port())
    }

    fn pair_with(&self, other: &Self) {
        self.config
            .trust
            .pin(other.config.identity.public_key(), other.id);
        self.live
            .host()
            .with_db(&mut |conn| {
                conn.execute(
                "INSERT INTO sync_peers(id, name, paired_at, public_key) VALUES (?1, ?1, 1, ?2)",
                rusqlite::params![other.id, other.config.identity.public_key().to_hex()],
            )?;
                Ok(())
            })
            .unwrap();
        self.live.refresh_peers();
    }
}

async fn until(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..300 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("never saw {what}");
}

/// Both devices dial each other at the same moment, so each holds two connections to the other.
/// `own_first`: whether each sees the connection it dialled before the one it accepted.
async fn both_dial(own_first: bool) {
    let (mut a, mut b) = (Device::new("dev-a").await, Device::new("dev-b").await);
    a.pair_with(&b);
    b.pair_with(&a);

    let a_to_b = dial(&a.config, b.addr()).await.unwrap();
    let b_to_a = dial(&b.config, a.addr()).await.unwrap();
    let Some(Incoming::Peer(at_b)) = b.listener.accept().await else {
        panic!("b did not get a's connection");
    };
    let Some(Incoming::Peer(at_a)) = a.listener.accept().await else {
        panic!("a did not get b's connection");
    };
    if own_first {
        a.live.adopt(a_to_b);
        a.live.accept(*at_a);
        b.live.adopt(b_to_a);
        b.live.accept(*at_b);
    } else {
        a.live.accept(*at_a);
        a.live.adopt(a_to_b);
        b.live.accept(*at_b);
        b.live.adopt(b_to_a);
    }

    // "dev-a" is the lower id: the connection it dialled stays on both devices.
    until("the lower id's connection kept on both", || {
        a.live.dialled_by_me("dev-b") == Some(true) && b.live.dialled_by_me("dev-a") == Some(false)
    })
    .await;
    until("a session over it", || {
        let up = |live: &Live, peer: &str| {
            live.snapshot()
                .get(peer)
                .is_some_and(|l| l.reachable && !l.syncing)
        };
        a.live.sessions() >= 1 && up(&a.live, "dev-b") && up(&b.live, "dev-a")
    })
    .await;
    // Nothing flips back later.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(a.live.dialled_by_me("dev-b"), Some(true));
    assert_eq!(b.live.dialled_by_me("dev-a"), Some(false));
}

#[tokio::test]
async fn when_both_devices_dial_the_connection_dialled_by_the_lower_id_is_kept() {
    both_dial(true).await;
}

#[tokio::test]
async fn the_same_holds_when_each_sees_the_other_connection_first() {
    both_dial(false).await;
}

#[tokio::test]
async fn a_connection_task_that_panics_does_not_leave_its_device_marked_connected() {
    let (a, mut b) = (Device::new("dev-a").await, Device::new("dev-b").await);
    a.pair_with(&b);
    b.pair_with(&a);
    let a_to_b = dial(&a.config, b.addr()).await.unwrap();
    let Some(Incoming::Peer(at_b)) = b.listener.accept().await else {
        panic!("b did not get a's connection");
    };
    a.fault.store(true, Ordering::SeqCst);
    a.live.adopt(a_to_b);
    b.live.accept(*at_b);

    until("the fault to strike", || !a.fault.load(Ordering::SeqCst)).await;
    until("the dead connection to be let go", || {
        a.live.dialled_by_me("dev-b").is_none()
            && a.live.snapshot().get("dev-b").is_some_and(|l| !l.reachable)
    })
    .await;
}

/// A dial that was still racing addresses when the other device's own connection was adopted must
/// not, when it gives up, mark the device as not nearby: a connection is open (DV2).
#[tokio::test]
async fn a_dial_that_fails_after_a_connection_was_adopted_leaves_the_device_reachable() {
    let (mut a, b) = (
        Device::paced("dev-a", Duration::from_millis(400)).await,
        Device::new("dev-b").await,
    );
    b.pair_with(&a);
    // An address that accepts and never answers: a's dial of b takes its whole 400 ms.
    let silent = tokio::net::TcpListener::bind((LOCALHOST, 0)).await.unwrap();
    let silent_addr = silent.local_addr().unwrap();
    let held = tokio::spawn(async move {
        let mut open = Vec::new();
        while let Ok((s, _)) = silent.accept().await {
            open.push(s);
        }
    });
    a.config.trust.pin(b.config.identity.public_key(), b.id);
    a.live
        .host()
        .with_db(&mut |conn| {
            conn.execute(
                "INSERT INTO sync_peers(id, name, paired_at, public_key) VALUES ('dev-b', 'dev-b', 1, ?1)",
                [b.config.identity.public_key().to_hex()],
            )?;
            conn.execute(
                "INSERT INTO sync_peer_addresses(device, address, last_ok_at) VALUES ('dev-b', ?1, NULL)",
                [silent_addr.to_string()],
            )?;
            Ok(())
        })
        .unwrap();
    a.live.refresh_peers();

    // b's own dial arrives while a is still dialling.
    let b_to_a = dial(&b.config, a.addr()).await.unwrap();
    let Some(Incoming::Peer(at_a)) = a.listener.accept().await else {
        panic!("a did not get b's connection");
    };
    a.live.accept(*at_a);
    b.live.adopt(b_to_a);
    until("a session over b's connection", || {
        a.live.snapshot().get("dev-b").is_some_and(|l| l.reachable)
    })
    .await;

    // a's dial gives up (400 ms); the connection is still open.
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert_eq!(a.live.dialled_by_me("dev-b"), Some(false));
    assert!(
        a.live.snapshot().get("dev-b").is_some_and(|l| l.reachable),
        "a failed dial must not clear the presence of an open connection"
    );
    held.abort();
}

/// Discovery finds nothing (the Mac that macOS keeps off the local network cannot browse), yet a
/// device is dialled at the address remembered for it, so it still syncs through the other
/// device's incoming connection (D14).
#[tokio::test]
async fn a_remembered_address_is_dialled_when_discovery_finds_nothing() {
    let (a, mut b) = (Device::new("dev-a").await, Device::new("dev-b").await);
    b.pair_with(&a);
    a.config.trust.pin(b.config.identity.public_key(), b.id);
    a.live
        .host()
        .with_db(&mut |conn| {
            conn.execute(
                "INSERT INTO sync_peers(id, name, paired_at, public_key) VALUES ('dev-b', 'dev-b', 1, ?1)",
                [b.config.identity.public_key().to_hex()],
            )?;
            conn.execute(
                "INSERT INTO sync_peer_addresses(device, address, last_ok_at) VALUES ('dev-b', ?1, NULL)",
                [b.addr().to_string()],
            )?;
            Ok(())
        })
        .unwrap();
    a.live.refresh_peers();

    let Some(Incoming::Peer(at_b)) = b.listener.accept().await else {
        panic!("b was not dialled");
    };
    b.live.accept(*at_b);
    until("a connection a dialled", || {
        a.live.dialled_by_me("dev-b") == Some(true)
            && a.live.snapshot().get("dev-b").is_some_and(|l| l.reachable)
    })
    .await;
}

fn denied() -> std::io::Result<()> {
    Err(std::io::Error::from(std::io::ErrorKind::HostUnreachable))
}

fn quickly() -> Rhythm {
    Rhythm {
        grace: Duration::from_millis(150),
        fast: Duration::from_millis(20),
        slow: Duration::from_millis(20),
        steady: Duration::from_millis(20),
    }
}

fn blocked_lines(device: &Device) -> i64 {
    let mut n = 0;
    device
        .live
        .host()
        .with_db(&mut |conn| {
            n = conn.query_row(
                "SELECT COUNT(*) FROM sync_activity WHERE kind = 'blocked'",
                [],
                |r| r.get(0),
            )?;
            Ok(())
        })
        .unwrap();
    n
}

/// A probe that keeps being refused marks the local network blocked once, tells the window, writes
/// one line in Activity, and clears (and tells the window again) when it works (D13).
#[tokio::test]
async fn a_probe_that_keeps_failing_blocks_once_and_a_working_one_clears_it() {
    let works = Arc::new(AtomicBool::new(false));
    let local = LocalNetwork::with_rhythm(
        {
            let works = Arc::clone(&works);
            move || {
                if works.load(Ordering::SeqCst) {
                    Ok(())
                } else {
                    denied()
                }
            }
        },
        quickly(),
    );
    let a = Device::build("dev-a", Duration::from_secs(3), Arc::clone(&local)).await;
    assert!(
        !local.blocked(),
        "the first refusals are the prompt being answered"
    );
    until("blocked", || local.blocked()).await;
    until("the window told", || {
        a.announced.load(Ordering::SeqCst) >= 1
    })
    .await;
    until("one line in Activity", || blocked_lines(&a) == 1).await;
    // Many more refusals later it is still one line and one announcement.
    let told = a.announced.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(blocked_lines(&a), 1);
    assert_eq!(a.announced.load(Ordering::SeqCst), told);

    works.store(true, Ordering::SeqCst);
    until("cleared", || !local.blocked()).await;
    until("the window told again", || {
        a.announced.load(Ordering::SeqCst) > told
    })
    .await;
    assert_eq!(blocked_lines(&a), 1);
}

/// With no probe a working network is never reported blocked, and dials to loopback (not a LAN
/// address) say nothing about the permission either way.
#[tokio::test]
async fn loopback_dials_do_not_touch_the_local_network_state() {
    let local = LocalNetwork::off();
    let a = Device::build("dev-a", Duration::from_millis(300), Arc::clone(&local)).await;
    let refused = std::net::TcpListener::bind((LOCALHOST, 0)).unwrap();
    let addr = refused.local_addr().unwrap();
    drop(refused);
    let race = super::dial::race(&a.config, vec![addr; 5], Duration::from_millis(300), &local);
    assert!(race.await.is_none());
    assert!(!local.blocked());
}
