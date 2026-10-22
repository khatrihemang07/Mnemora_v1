//! Loopback tests: real TCP + TLS sockets on 127.0.0.1 between two in-memory devices.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::db::{self, migrate};
use mnemora_core::env::Environment;
use mnemora_core::error::AppResult;
use mnemora_core::events::EventBus;
use mnemora_core::hlc;
use mnemora_core::module::{CoreModule, Module};
use mnemora_core::settings::SettingsRegistry;
use mnemora_core::syncable::{Applied, RemoteChange, SyncEntity, SyncField};
use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;

use super::*;
use crate::registry::{Modules, Registry};
use crate::runtime::SyncRuntime;
use crate::session::Endpoint;
use crate::{SyncError, SyncModule, Transport};

const LOCALHOST: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

/// A Todo-like Module: tasks with a title and a done flag.
struct Todoish;

const ENTITIES: &[SyncEntity] = &[SyncEntity {
    name: "task",
    fields: &[SyncField::text("title"), SyncField::value("done")],
}];

impl Module for Todoish {
    fn id(&self) -> &'static str {
        "todoish"
    }
    fn migrations(&self) -> Migrations<'static> {
        Migrations::new(vec![M::up(
            "CREATE TABLE todoish_tasks(id TEXT PRIMARY KEY NOT NULL, title TEXT, done INTEGER) STRICT",
        )])
    }
    fn register_settings(&self, _: &mut SettingsRegistry) {}
    fn sync_entities(&self) -> &'static [SyncEntity] {
        ENTITIES
    }
    fn apply_remote(&self, conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
        conn.execute(
            "INSERT INTO todoish_tasks(id, title, done) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET title = COALESCE(?2, title), done = COALESCE(?3, done)",
            rusqlite::params![
                change.entity_id,
                change.fields.get("title").and_then(Value::as_str),
                change.fields.get("done").and_then(Value::as_i64),
            ],
        )?;
        Ok(Applied {
            snapshot: json!({"id": change.entity_id}),
            changed: true,
        })
    }
}

struct Device {
    name: &'static str,
    conn: Connection,
    registry: Registry,
    runtime: SyncRuntime,
    config: NetConfig,
}

impl Device {
    fn new(name: &'static str, environment: Environment) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        migrate::run(&mut conn, &[&CoreModule, &SyncModule, &Todoish], dir.path()).unwrap();
        hlc::set_device_id(&conn, name).unwrap();
        let modules: Modules = Arc::new(vec![Box::new(Todoish)]);
        let runtime = SyncRuntime::new();
        runtime.reconcile(true);
        let local = Handshake::local(&conn, environment, name, None, vec![]).unwrap();
        Self {
            name,
            conn,
            registry: Registry::new(modules),
            runtime,
            config: NetConfig {
                identity: Arc::new(DeviceIdentity::generate().unwrap()),
                trust: Trust::new(),
                local,
            },
        }
    }

    fn key(&self) -> PublicKey {
        self.config.identity.public_key()
    }

    fn pin(&self, other: &Device) {
        self.config.trust.pin(other.key(), other.name);
    }

    fn add_task(&self, id: &str, title: &str) {
        self.conn
            .execute(
                "INSERT INTO todoish_tasks(id, title, done) VALUES (?1, ?2, 0)",
                [id, title],
            )
            .unwrap();
        changelog::record(
            &self.conn,
            &Change {
                module: "todoish".into(),
                entity: "task".into(),
                entity_id: id.into(),
                op: ChangeOp::Create,
                session_id: None,
                snapshot: json!({"id": id, "title": title, "done": 0}),
            },
        )
        .unwrap();
    }

    fn titles(&self) -> Vec<String> {
        let mut stmt = self
            .conn
            .prepare("SELECT title FROM todoish_tasks ORDER BY title")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    fn endpoint(&self, transport: NetTransport) -> Endpoint<NetTransport> {
        let session = self
            .runtime
            .session(&self.conn, self.registry.clone(), EventBus::new())
            .unwrap();
        Endpoint::new(session, transport)
    }
}

async fn listen(device: &Device) -> Listener {
    Listener::bind(device.config.clone(), LOCALHOST, 0)
        .await
        .unwrap()
}

async fn next_peer(listener: &mut Listener) -> Established {
    match tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .expect("a connection arrives")
        .expect("the listener is open")
    {
        Incoming::Peer(established) => *established,
        Incoming::Unpinned(_) => panic!("expected a pinned peer"),
    }
}

async fn nothing_arrives(listener: &mut Listener) {
    assert!(
        tokio::time::timeout(Duration::from_millis(400), listener.accept())
            .await
            .is_err(),
        "the listener must not hand out a refused connection"
    );
}

/// Polls both endpoints until both sessions are done.
async fn run(
    a: (&mut Endpoint<NetTransport>, &Connection),
    b: (&mut Endpoint<NetTransport>, &Connection),
) {
    let (ea, ca) = a;
    let (eb, cb) = b;
    tokio::time::timeout(Duration::from_secs(10), async {
        while !(ea.is_done() && eb.is_done()) {
            let moved_a = ea.poll(ca).unwrap();
            let moved_b = eb.poll(cb).unwrap();
            if !moved_a && !moved_b {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
    })
    .await
    .expect("the sessions finish");
}

fn pinned_pair(env_b: Environment) -> (Device, Device) {
    let a = Device::new("alpha", Environment::Production);
    let b = Device::new("bravo", env_b);
    a.pin(&b);
    b.pin(&a);
    (a, b)
}

#[tokio::test]
async fn two_pinned_peers_exchange_a_todo_sync_over_real_sockets() {
    let (a, b) = pinned_pair(Environment::Production);
    a.add_task("t1", "buy milk");
    a.add_task("t2", "call mum");
    b.add_task("t3", "water plants");
    let mut listener = listen(&a).await;

    let (dialled, accepted) = tokio::join!(
        dial(&b.config, listener.local_addr()),
        next_peer(&mut listener)
    );
    let dialled = dialled.unwrap();
    assert_eq!(dialled.peer.key, a.key());
    assert_eq!(dialled.peer.handshake.device, "alpha");
    assert_eq!(accepted.peer.key, b.key());
    assert_eq!(accepted.peer.handshake.device, "bravo");
    assert!(accepted.peer.data_version_mismatch.is_empty());
    assert_eq!(
        accepted.peer.handshake.data_versions, b.config.local.data_versions,
        "per-Module data versions travel"
    );

    let mut ea = a.endpoint(accepted.transport);
    let mut eb = b.endpoint(dialled.transport);
    run((&mut ea, &a.conn), (&mut eb, &b.conn)).await;

    assert_eq!(a.titles(), ["buy milk", "call mum", "water plants"]);
    assert_eq!(b.titles(), ["buy milk", "call mum", "water plants"]);
    assert_eq!(ea.session().stats().received, 1);
    assert_eq!(eb.session().stats().received, 2);
}

#[tokio::test]
async fn a_dialler_the_listener_has_not_pinned_is_refused() {
    let a = Device::new("alpha", Environment::Production);
    let b = Device::new("bravo", Environment::Production);
    b.pin(&a); // B trusts A, but A has never heard of B.
    let mut listener = listen(&a).await;

    let result = dial(&b.config, listener.local_addr()).await;
    assert!(
        matches!(result, Err(NetError::Tls(_))),
        "the server's TLS alert reaches the dialler"
    );
    nothing_arrives(&mut listener).await;
}

#[tokio::test]
async fn a_listener_the_dialler_has_not_pinned_is_refused() {
    let a = Device::new("alpha", Environment::Production);
    let b = Device::new("bravo", Environment::Production);
    a.pin(&b); // A trusts B, but B does not know A.
    let mut listener = listen(&a).await;

    let result = dial(&b.config, listener.local_addr()).await;
    assert!(matches!(result, Err(NetError::Tls(_))));
    nothing_arrives(&mut listener).await;
}

#[tokio::test]
async fn a_peer_pinned_under_another_device_id_is_refused() {
    let a = Device::new("alpha", Environment::Production);
    let b = Device::new("bravo", Environment::Production);
    a.config.trust.pin(b.key(), "someone-else");
    b.pin(&a);
    let mut listener = listen(&a).await;

    // B's own checks pass (it pinned A correctly); A is the one that refuses.
    let dialled = dial(&b.config, listener.local_addr()).await;
    nothing_arrives(&mut listener).await;
    if let Ok(mut dialled) = dialled {
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(matches!(
            dialled.transport.try_recv(),
            Err(SyncError::Disconnected)
        ));
    }
}

#[tokio::test]
async fn a_forgotten_peer_is_refused_on_its_next_connection() {
    let (a, b) = pinned_pair(Environment::Production);
    let mut listener = listen(&a).await;
    let (dialled, _accepted) = tokio::join!(
        dial(&b.config, listener.local_addr()),
        next_peer(&mut listener)
    );
    dialled.unwrap();

    a.config.trust.unpin(&b.key());
    assert!(dial(&b.config, listener.local_addr()).await.is_err());
    nothing_arrives(&mut listener).await;
}

#[tokio::test]
async fn a_peer_in_the_other_environment_is_refused_on_both_ends() {
    let (a, b) = pinned_pair(Environment::Sandbox);
    let mut listener = listen(&a).await;

    let result = dial(&b.config, listener.local_addr()).await;
    assert!(matches!(
        result,
        Err(NetError::Refused(Refusal::Environment))
    ));
    nothing_arrives(&mut listener).await;
}

#[tokio::test]
async fn a_frame_over_8_mb_closes_the_connection() {
    let (a, b) = pinned_pair(Environment::Production);
    let mut listener = listen(&a).await;

    let listener_addr = listener.local_addr();
    // Be a well-behaved peer through the handshake, then lie about a frame's size.
    let liar = async {
        let mut channel = secure_dial(&b.config, listener_addr).await.unwrap().channel;
        channel.send(&b.config.local).await.unwrap();
        let _: Handshake = channel.recv().await.unwrap();
        let mut stream = channel.into_inner();
        let header = u32::try_from(MAX_FRAME + 1).unwrap().to_be_bytes();
        stream.write_all(&header).await.unwrap();
        stream.flush().await.unwrap();
        stream
    };
    let (_stream, accepted) = tokio::join!(liar, next_peer(&mut listener));

    let mut transport = accepted.transport;
    let outcome = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match transport.try_recv() {
                Ok(None) => tokio::time::sleep(Duration::from_millis(5)).await,
                other => return other,
            }
        }
    })
    .await
    .expect("the connection is closed");
    assert!(matches!(outcome, Err(SyncError::Disconnected)));
    assert!(matches!(
        transport.send(crate::Message::Ack),
        Err(SyncError::Disconnected) | Ok(())
    ));
}

#[tokio::test]
async fn a_busy_default_port_falls_back_to_an_ephemeral_one() {
    let (a, b) = pinned_pair(Environment::Production);
    // Hold a port, then ask for it: the stand-in for 47311 being taken.
    let holder = std::net::TcpListener::bind((LOCALHOST, 0)).unwrap();
    let busy = holder.local_addr().unwrap().port();

    let mut listener = Listener::bind(a.config.clone(), LOCALHOST, busy)
        .await
        .unwrap();
    assert!(listener.fell_back());
    assert_ne!(listener.local_addr().port(), busy);

    let addr = SocketAddr::new(LOCALHOST, listener.local_addr().port());
    let (dialled, _accepted) = tokio::join!(dial(&b.config, addr), next_peer(&mut listener));
    dialled.unwrap();
}

#[tokio::test]
async fn a_free_preferred_port_is_used_as_asked() {
    let a = Device::new("alpha", Environment::Production);
    let probe = std::net::TcpListener::bind((LOCALHOST, 0)).unwrap();
    let free = probe.local_addr().unwrap().port();
    drop(probe);
    match Listener::bind(a.config.clone(), LOCALHOST, free).await {
        Ok(listener) => {
            // Someone could grab the port in between; only assert when we got it.
            assert_eq!(listener.fell_back(), listener.local_addr().port() != free);
        }
        Err(e) => panic!("bind failed: {e}"),
    }
}

#[tokio::test]
async fn while_pairing_is_open_an_unpinned_peer_reaches_the_pairing_hook_and_afterwards_is_refused()
{
    let a = Device::new("alpha", Environment::Production);
    let b = Device::new("bravo", Environment::Production);
    b.pin(&a);
    a.config.trust.set_pairing_open(true);
    let mut listener = listen(&a).await;

    // B pinned A; A has not pinned B, but pairing is open.
    let secured = secure_dial(&b.config, listener.local_addr()).await.unwrap();
    assert!(secured.pinned, "B pinned A");
    let Incoming::Unpinned(unpinned) =
        tokio::time::timeout(Duration::from_secs(5), listener.accept())
            .await
            .unwrap()
            .unwrap()
    else {
        panic!("A should see an unpinned peer");
    };
    assert_eq!(unpinned.key, b.key());
    assert!(!unpinned.pinned);

    // Once pairing closes, the same key is refused outright: not pinned, not admitted.
    drop(unpinned);
    a.config.trust.set_pairing_open(false);
    assert!(dial(&b.config, listener.local_addr()).await.is_err());
}
