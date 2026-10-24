//! Pairing over real loopback sockets between two in-memory devices: right code, wrong code,
//! tries used up, expiry, deny, cancel, other Environment, other data versions, name clashes,
//! joining a group, and the port closing with Sync.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use mnemora_core::db::{self, migrate};
use mnemora_core::env::Environment;
use mnemora_core::hlc;
use mnemora_core::module::CoreModule;
use rusqlite::Connection;

use super::msg::Who;
use super::view::{Older, PairingError, PairingView, Platform};
use super::{Paired, PairingHost, Stored, Timing};
use crate::discovery::Fixed;
use crate::net::{DeviceIdentity, Handshake, PublicKey};
use crate::{Network, NetworkOptions, SyncError, SyncModule, peers};

const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

/// The app's side of pairing, over an in-memory database.
struct Host {
    device: String,
    name: String,
    environment: Environment,
    conn: Mutex<Connection>,
    versions: Mutex<Option<BTreeMap<String, usize>>>,
    visible: bool,
    views: Mutex<Vec<PairingView>>,
}

impl Host {
    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn versions(&self, conn: &Connection) -> BTreeMap<String, usize> {
        self.versions
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| migrate::versions(conn).unwrap())
    }
}

impl PairingHost for Host {
    fn me(&self) -> Result<Who, SyncError> {
        let conn = self.conn();
        Ok(Who {
            device: self.device.clone(),
            name: self.name.clone(),
            platform: Platform::Desktop,
            key: String::new(),
            environment: self.environment,
            group: peers::group(&conn)?,
            data_versions: self.versions(&conn),
            members: peers::members(&conn)?,
        })
    }

    fn handshake(&self) -> Result<Handshake, SyncError> {
        let conn = self.conn();
        let group = peers::group(&conn)?;
        Handshake::local(&conn, self.environment, &self.device, group, vec![])
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn pinned(&self) -> Vec<(PublicKey, String)> {
        peers::pinned(&self.conn()).unwrap()
    }

    fn store(&self, paired: &Paired) -> Result<Stored, SyncError> {
        peers::store_paired(&mut self.conn(), &self.device, &self.name, paired)
    }

    fn show(&self, view: &PairingView) {
        self.views.lock().unwrap().push(view.clone());
    }
}

/// Pairing tests do not sync anything: an empty registry, and no changes to push.
impl crate::Host for Host {
    fn with_db(
        &self,
        f: &mut dyn FnMut(&Connection) -> Result<(), SyncError>,
    ) -> Result<(), SyncError> {
        f(&self.conn())
    }

    fn registry(&self) -> crate::Registry {
        crate::Registry::new(Arc::new(Vec::new()))
    }

    fn bus(&self) -> mnemora_core::events::EventBus {
        mnemora_core::events::EventBus::new()
    }

    fn automatic(&self) -> bool {
        true
    }

    fn local_changes(&self) -> Arc<tokio::sync::Notify> {
        Arc::default()
    }

    fn status_changed(&self) {}
}

struct Dev {
    host: Arc<Host>,
    network: Network,
    found: Arc<Fixed>,
}

impl Dev {
    async fn new(name: &str, environment: Environment) -> Self {
        Self::with(name, environment, Timing::default(), None).await
    }

    async fn with(
        name: &str,
        environment: Environment,
        timing: Timing,
        versions: Option<BTreeMap<String, usize>>,
    ) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        migrate::run(&mut conn, &[&CoreModule, &SyncModule], dir.path()).unwrap();
        let device = format!("dev-{name}");
        hlc::set_device_id(&conn, &device).unwrap();
        let host = Arc::new(Host {
            device,
            name: name.to_owned(),
            environment,
            conn: Mutex::new(conn),
            versions: Mutex::new(versions),
            visible: true,
            views: Mutex::default(),
        });
        let found = Fixed::new();
        let options = NetworkOptions {
            ip: LOCALHOST,
            port: 0,
            ip6: None,
            timing: Timing {
                search: Duration::from_millis(800),
                ..timing
            },
            discovery: found.clone(),
            pace: crate::Pace::default(),
            local_network: crate::local_network::LocalNetwork::off(),
        };
        let identity = Arc::new(DeviceIdentity::generate().unwrap());
        let network = Network::start(host.clone(), identity, options)
            .await
            .unwrap();
        Self {
            host,
            network,
            found,
        }
    }

    fn addr(&self) -> SocketAddr {
        SocketAddr::new(LOCALHOST, self.network.addr().port())
    }

    fn state(&self) -> PairingView {
        self.network.pairing().state()
    }

    /// Waits until the view satisfies `want`, and returns it.
    async fn until(&self, what: &str, want: impl Fn(&PairingView) -> bool) -> PairingView {
        for _ in 0..600 {
            let view = self.state();
            if want(&view) {
                return view;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!(
            "{}: never saw {what}; last view {:?}",
            self.host.name,
            self.state()
        );
    }

    /// The code this device shows, as 8 digits.
    fn code(&self) -> String {
        match self.state() {
            PairingView::Showing { code, .. } => code.replace(' ', ""),
            other => panic!("not showing a code: {other:?}"),
        }
    }

    fn peer_names(&self) -> Vec<String> {
        let mut names: Vec<String> = peers::list(&self.host.conn())
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        names.sort();
        names
    }

    fn group(&self) -> Option<String> {
        peers::group(&self.host.conn()).unwrap()
    }
}

/// `shower` shows a code, `typer` types it, the person on `shower` allows.
async fn pair(shower: &Dev, typer: &Dev) -> (PairingView, PairingView) {
    shower.network.pairing().show().unwrap();
    typer.found.set(vec![shower.addr()]);
    typer.network.pairing().enter(&shower.code()).unwrap();
    shower
        .until("the question", |v| matches!(v, PairingView::Asking { .. }))
        .await;
    shower.network.pairing().decide(true);
    let a = shower
        .until("paired", |v| matches!(v, PairingView::Paired { .. }))
        .await;
    let b = typer
        .until("paired", |v| matches!(v, PairingView::Paired { .. }))
        .await;
    (a, b)
}

fn failed(view: &PairingView) -> &PairingError {
    match view {
        PairingView::Failed { error } => error,
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[tokio::test]
async fn the_right_code_pairs_both_devices_and_each_lists_the_other() {
    let (mac, phone) = (
        Dev::new("Hemang's MacBook Pro", Environment::Production).await,
        Dev::new("Pixel 8", Environment::Production).await,
    );
    let (on_mac, on_phone) = pair(&mac, &phone).await;
    assert_eq!(
        on_mac,
        PairingView::Paired {
            name: "Pixel 8".into(),
            platform: Platform::Desktop,
            renamed: false,
            also_joined: vec![]
        }
    );
    assert!(
        matches!(on_phone, PairingView::Paired { ref name, .. } if name == "Hemang's MacBook Pro")
    );
    assert_eq!(mac.peer_names(), ["Pixel 8"]);
    assert_eq!(phone.peer_names(), ["Hemang's MacBook Pro"]);
    assert!(
        mac.group().is_some(),
        "a group is made at the first pairing"
    );
    assert_eq!(mac.group(), phone.group(), "and both belong to it");
    // Closed again: nothing shown, nothing admitted, nothing announced as open.
    assert!(!mac.network.pairing().state_is_open());
    assert_eq!(mac.network.pairing().state(), on_mac);
}

#[tokio::test]
async fn a_wrong_code_reports_tries_left_and_the_right_one_still_works() {
    let (a, b) = (
        Dev::new("Mac", Environment::Production).await,
        Dev::new("Phone", Environment::Production).await,
    );
    a.network.pairing().show().unwrap();
    b.found.set(vec![a.addr()]);
    let real = a.code();
    let wrong = if real == "00000000" {
        "11111111"
    } else {
        "00000000"
    };
    b.network.pairing().enter(wrong).unwrap();
    let seen = b
        .until("a wrong code", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert_eq!(
        failed(&seen),
        &PairingError::WrongCode {
            tries_left: 2,
            name: "Mac".into()
        }
    );
    let shown = a
        .until("one try gone", |v| {
            matches!(v, PairingView::Showing { tries_left: 2, .. })
        })
        .await;
    assert!(matches!(shown, PairingView::Showing { .. }));
    assert!(a.peer_names().is_empty() && b.peer_names().is_empty());
    b.network.pairing().enter(&real).unwrap();
    a.until("the question", |v| matches!(v, PairingView::Asking { .. }))
        .await;
}

#[tokio::test]
async fn three_wrong_tries_kill_the_code() {
    let (a, b) = (
        Dev::new("Mac", Environment::Production).await,
        Dev::new("Phone", Environment::Production).await,
    );
    a.network.pairing().show().unwrap();
    b.found.set(vec![a.addr()]);
    let real = a.code();
    let wrong = if real == "00000000" {
        "11111111"
    } else {
        "00000000"
    };
    for expected in [2, 1, 0] {
        b.network.pairing().enter(wrong).unwrap();
        let seen = b
            .until("a wrong code", |v| {
                matches!(v, PairingView::Failed { error: PairingError::WrongCode { tries_left, .. } } if *tries_left == expected)
            })
            .await;
        assert!(matches!(seen, PairingView::Failed { .. }));
    }
    let dead = a
        .until("a dead code", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert_eq!(failed(&dead), &PairingError::TriesUsed);
    // The right code no longer gets in.
    b.network.pairing().enter(&real).unwrap();
    let seen = b
        .until("a failure", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert_eq!(failed(&seen), &PairingError::ConnectionLost);
    assert!(a.peer_names().is_empty() && b.peer_names().is_empty());
}

#[tokio::test]
async fn an_expired_code_is_refused_and_says_so() {
    let short = Timing {
        code_life: Duration::from_millis(300),
        ..Timing::default()
    };
    let a = Dev::with("Mac", Environment::Production, short, None).await;
    let b = Dev::new("Phone", Environment::Production).await;
    a.network.pairing().show().unwrap();
    b.found.set(vec![a.addr()]);
    let real = a.code();
    let dead = a
        .until("expiry", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert_eq!(failed(&dead), &PairingError::CodeExpired { name: None });
    b.network.pairing().enter(&real).unwrap();
    let seen = b
        .until("a failure", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert!(matches!(
        failed(&seen),
        PairingError::ConnectionLost | PairingError::CodeExpired { .. }
    ));
    assert!(a.peer_names().is_empty() && b.peer_names().is_empty());
}

#[tokio::test]
async fn pairing_by_address_with_an_expired_code_says_it_expired() {
    let short = Timing {
        code_life: Duration::from_millis(300),
        ..Timing::default()
    };
    let a = Dev::with("Mac", Environment::Production, short, None).await;
    let b = Dev::new("Phone", Environment::Production).await;
    a.network.pairing().show().unwrap();
    let real = a.code();
    a.until("expiry", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    b.network
        .pairing()
        .enter_at(&a.addr().to_string(), &real)
        .unwrap();
    let seen = b
        .until("a failure", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert_eq!(failed(&seen), &PairingError::CodeExpired { name: None });
    assert!(a.peer_names().is_empty() && b.peer_names().is_empty());
}

#[tokio::test]
async fn running_out_of_time_while_asking_tells_the_other_device() {
    let short = Timing {
        code_life: Duration::from_millis(600),
        ..Timing::default()
    };
    let a = Dev::with("Mac", Environment::Production, short, None).await;
    let b = Dev::with("Phone", Environment::Production, short, None).await;
    a.network.pairing().show().unwrap();
    b.found.set(vec![a.addr()]);
    b.network.pairing().enter(&a.code()).unwrap();
    a.until("the question", |v| matches!(v, PairingView::Asking { .. }))
        .await;
    let seen = b
        .until("a failure", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert_eq!(
        failed(&seen),
        &PairingError::CodeExpired {
            name: Some("Mac".into())
        }
    );
    a.until("expiry", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert!(a.peer_names().is_empty() && b.peer_names().is_empty());
}

#[tokio::test]
async fn deny_tells_the_other_device_and_pairs_nothing() {
    let (a, b) = (
        Dev::new("Mac", Environment::Production).await,
        Dev::new("Phone", Environment::Production).await,
    );
    a.network.pairing().show().unwrap();
    b.found.set(vec![a.addr()]);
    b.network.pairing().enter(&a.code()).unwrap();
    let asked = a
        .until("the question", |v| matches!(v, PairingView::Asking { .. }))
        .await;
    assert!(matches!(asked, PairingView::Asking { ref name, .. } if name == "Phone"));
    assert!(matches!(
        b.state(),
        PairingView::Waiting { ref name } if name == "Mac"
    ));
    a.network.pairing().decide(false);
    let seen = b
        .until("a failure", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert_eq!(
        failed(&seen),
        &PairingError::Declined { name: "Mac".into() }
    );
    a.until("closed", |v| matches!(v, PairingView::Idle)).await;
    assert!(a.peer_names().is_empty() && b.peer_names().is_empty());
}

#[tokio::test]
async fn cancelling_while_asked_tells_the_typing_device() {
    let (a, b) = (
        Dev::new("Mac", Environment::Production).await,
        Dev::new("Phone", Environment::Production).await,
    );
    a.network.pairing().show().unwrap();
    b.found.set(vec![a.addr()]);
    b.network.pairing().enter(&a.code()).unwrap();
    a.until("the question", |v| matches!(v, PairingView::Asking { .. }))
        .await;
    a.network.pairing().cancel();
    let seen = b
        .until("a failure", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert_eq!(
        failed(&seen),
        &PairingError::Cancelled {
            name: Some("Mac".into())
        }
    );
}

#[tokio::test]
async fn the_typing_device_leaving_returns_the_showing_one_to_its_code() {
    let (a, b) = (
        Dev::new("Mac", Environment::Production).await,
        Dev::new("Phone", Environment::Production).await,
    );
    a.network.pairing().show().unwrap();
    b.found.set(vec![a.addr()]);
    let code = a.code();
    b.network.pairing().enter(&code).unwrap();
    a.until("the question", |v| matches!(v, PairingView::Asking { .. }))
        .await;
    b.network.pairing().cancel();
    let back = a
        .until("the code again", |v| {
            matches!(v, PairingView::Showing { .. })
        })
        .await;
    assert!(matches!(back, PairingView::Showing { tries_left: 3, .. }));
}

#[tokio::test]
async fn sandbox_and_production_refuse_each_other() {
    let (prod, sandbox) = (
        Dev::new("Mac", Environment::Production).await,
        Dev::new("Phone", Environment::Sandbox).await,
    );
    prod.network.pairing().show().unwrap();
    sandbox.found.set(vec![prod.addr()]);
    sandbox.network.pairing().enter(&prod.code()).unwrap();
    let seen = sandbox
        .until("a failure", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert_eq!(
        failed(&seen),
        &PairingError::DifferentEnvironment {
            theirs: Environment::Production
        }
    );
    let shown = prod
        .until("a notice", |v| {
            matches!(
                v,
                PairingView::Showing {
                    notice: Some(_),
                    ..
                }
            )
        })
        .await;
    assert!(matches!(
        shown,
        PairingView::Showing {
            notice: Some(PairingError::DifferentEnvironment {
                theirs: Environment::Sandbox
            }),
            ..
        }
    ));
    assert!(prod.peer_names().is_empty() && sandbox.peer_names().is_empty());
}

#[tokio::test]
async fn devices_on_different_data_versions_do_not_pair() {
    let a = Dev::new("Mac", Environment::Production).await;
    let mut newer = migrate::versions(&a.host.conn()).unwrap();
    *newer.get_mut("core").unwrap() += 1;
    let b = Dev::with(
        "Phone",
        Environment::Production,
        Timing::default(),
        Some(newer),
    )
    .await;
    a.network.pairing().show().unwrap();
    b.found.set(vec![a.addr()]);
    b.network.pairing().enter(&a.code()).unwrap();
    let seen = b
        .until("a failure", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert_eq!(
        failed(&seen),
        &PairingError::VersionMismatch {
            name: "Mac".into(),
            older: Older::Them,
            older_here: vec![],
            older_there: vec!["core".into()],
        }
    );
    let shown = a
        .until("a notice", |v| {
            matches!(
                v,
                PairingView::Showing {
                    notice: Some(_),
                    ..
                }
            )
        })
        .await;
    assert!(matches!(
        shown,
        PairingView::Showing { notice: Some(PairingError::VersionMismatch { ref name, older: Older::Me, ref older_here, ref older_there }), .. } if name == "Phone" && older_here == &["core".to_owned()] && older_there.is_empty()
    ));
    assert!(a.peer_names().is_empty() && b.peer_names().is_empty());
}

#[tokio::test]
async fn a_name_that_clashes_gets_a_number() {
    let (a, b) = (
        Dev::new("Pixel 8", Environment::Production).await,
        Dev::new("PIXEL 8", Environment::Production).await,
    );
    let (on_a, on_b) = pair(&a, &b).await;
    assert!(
        matches!(on_a, PairingView::Paired { ref name, renamed: true, .. } if name == "PIXEL 8 (2)")
    );
    assert!(
        matches!(on_b, PairingView::Paired { ref name, renamed: true, .. } if name == "Pixel 8 (2)")
    );
}

#[tokio::test]
async fn pairing_with_one_member_joins_the_whole_group() {
    let (mac, air, phone) = (
        Dev::new("MacBook Pro", Environment::Production).await,
        Dev::new("MacBook Air", Environment::Production).await,
        Dev::new("Pixel 8", Environment::Production).await,
    );
    pair(&mac, &air).await;
    let (_, on_phone) = pair(&mac, &phone).await;
    assert!(matches!(
        on_phone,
        PairingView::Paired { ref also_joined, .. } if also_joined == &["MacBook Air"]
    ));
    assert_eq!(phone.peer_names(), ["MacBook Air", "MacBook Pro"]);
    assert_eq!(phone.group(), mac.group());
    assert_eq!(phone.group(), air.group());
    assert_eq!(mac.peer_names(), ["MacBook Air", "Pixel 8"]);
}

#[tokio::test]
async fn two_devices_in_different_groups_do_not_pair() {
    let (a, b, c, d) = (
        Dev::new("A", Environment::Production).await,
        Dev::new("B", Environment::Production).await,
        Dev::new("C", Environment::Production).await,
        Dev::new("D", Environment::Production).await,
    );
    pair(&a, &b).await;
    pair(&c, &d).await;
    a.network.pairing().show().unwrap();
    c.found.set(vec![a.addr()]);
    c.network.pairing().enter(&a.code()).unwrap();
    let seen = c
        .until("a failure", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert_eq!(failed(&seen), &PairingError::DifferentGroups);
    assert_eq!(a.peer_names(), ["B"]);
    assert_eq!(c.peer_names(), ["D"]);
}

#[tokio::test]
async fn nothing_nearby_is_reported_when_no_device_shows_a_code() {
    let b = Dev::new("Phone", Environment::Production).await;
    b.network.pairing().enter("12345678").unwrap();
    assert_eq!(b.state(), PairingView::Searching);
    let seen = b
        .until("a failure", |v| matches!(v, PairingView::Failed { .. }))
        .await;
    assert_eq!(failed(&seen), &PairingError::NothingNearby);
}

#[tokio::test]
async fn a_code_must_have_eight_digits() {
    let b = Dev::new("Phone", Environment::Production).await;
    for bad in ["", "1234567", "1234 abcd", "123456789"] {
        assert!(b.network.pairing().enter(bad).is_err(), "{bad:?}");
    }
    assert_eq!(b.state(), PairingView::Idle);
}

#[tokio::test]
async fn the_pairing_flag_is_announced_only_while_a_code_is_shown() {
    let a = Dev::new("Mac", Environment::Production).await;
    let advert = a.found.advertised().unwrap();
    assert!(!advert.pairing_open);
    a.network.pairing().show().unwrap();
    assert!(a.found.advertised().unwrap().pairing_open);
    a.network.pairing().cancel();
    assert!(!a.found.advertised().unwrap().pairing_open);
}

#[tokio::test]
async fn new_code_makes_a_different_code_with_fresh_tries() {
    let a = Dev::new("Mac", Environment::Production).await;
    a.network.pairing().show().unwrap();
    let first = a.code();
    a.network.pairing().new_code().unwrap();
    assert_ne!(a.code(), first, "a new code (1 in 10^8 to collide)");
    assert!(matches!(
        a.state(),
        PairingView::Showing { tries_left: 3, .. }
    ));
}

#[tokio::test]
async fn dropping_the_network_closes_the_port_and_stops_announcing() {
    let a = Dev::new("Mac", Environment::Production).await;
    let addr = a.addr();
    let found = a.found.clone();
    a.network.pairing().show().unwrap();
    drop(a);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        tokio::net::TcpStream::connect(addr).await.is_err(),
        "port still open"
    );
    assert_eq!(found.advertised(), None);
}

/// The tries are spent under the same lock that checks the proof, so a proof checked after the
/// last try went (a parallel connection) is never even looked at.
#[tokio::test]
async fn a_proof_is_not_checked_once_the_tries_are_spent() {
    use super::hub::ShowSide;
    use super::proto::{Attempt, ShowerPort};

    let a = Dev::new("Mac", Environment::Production).await;
    a.network.pairing().show().unwrap();
    let code = a.code();
    let side = ShowSide::new(a.network.pairing());
    assert_eq!(side.attempt(&code, &|| false), Attempt::Wrong(2));
    assert_eq!(side.attempt("00000000x", &|| true), Attempt::Dead);
    assert_eq!(side.attempt(&code, &|| false), Attempt::Wrong(1));
    assert_eq!(side.attempt(&code, &|| false), Attempt::Wrong(0));
    let checked = std::cell::Cell::new(false);
    assert_eq!(
        side.attempt(&code, &|| {
            checked.set(true);
            true
        }),
        Attempt::Dead
    );
    assert!(!checked.get());
}
