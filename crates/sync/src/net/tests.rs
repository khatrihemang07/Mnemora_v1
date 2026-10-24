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
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
            events: Vec::new(),
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
                local: LocalHandshake::fixed(local),
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

/// The ways a server's refusal of a dialler's certificate reaches the dialler (see its test).
fn refused_by_the_server(e: &NetError) -> bool {
    use std::io::ErrorKind::{BrokenPipe, ConnectionAborted, ConnectionReset, UnexpectedEof};
    match e {
        NetError::Tls(_) | NetError::Closed => true,
        NetError::Io(io) => matches!(
            io.kind(),
            BrokenPipe | ConnectionReset | ConnectionAborted | UnexpectedEof
        ),
        _ => false,
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
        accepted.peer.handshake.data_versions,
        b.config.local.get().unwrap().data_versions,
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
    // TLS 1.3 lets the dialler finish its side before the server has judged its certificate, so
    // the refusal reaches it as the server's alert (Tls) or, when the server has already closed
    // and the dialler writes its handshake or reads first, as the closed line (broken pipe, reset,
    // end of stream). Which of them depends on timing; what matters is that it failed.
    assert!(
        matches!(&result, Err(e) if refused_by_the_server(e)),
        "the dialler must be turned away, got {:?}",
        result.as_ref().err()
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
        channel.send(&b.config.local.get().unwrap()).await.unwrap();
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

    // B pinned A; A has not pinned B, but pairing is open. The dialler speaks first.
    let mut secured = secure_dial(&b.config, listener.local_addr()).await.unwrap();
    assert!(secured.pinned, "B pinned A");
    secured.channel.send(&json!({"pake": 1})).await.unwrap();
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

#[tokio::test]
async fn a_pinned_peer_gets_a_normal_session_while_pairing_is_open() {
    let (a, b) = pinned_pair(Environment::Production);
    a.config.trust.set_pairing_open(true);
    b.config.trust.set_pairing_open(true);
    a.add_task("t1", "buy milk");
    let mut listener = listen(&a).await;

    let (dialled, accepted) = tokio::join!(
        tokio::time::timeout(
            Duration::from_secs(2),
            dial(&b.config, listener.local_addr())
        ),
        next_peer(&mut listener)
    );
    let dialled = dialled.expect("no wait for pairing to drop us").unwrap();
    let mut ea = a.endpoint(accepted.transport);
    let mut eb = b.endpoint(dialled.transport);
    run((&mut ea, &a.conn), (&mut eb, &b.conn)).await;
    assert_eq!(b.titles(), ["buy milk"]);
}

/// "Check" (a probe) only listens: a pinned device must still be reachable by it while the other
/// one has its pair sheet open (DV6 review).
#[tokio::test]
async fn a_pinned_probe_is_answered_while_pairing_is_open() {
    let (a, b) = pinned_pair(Environment::Production);
    a.config.trust.set_pairing_open(true);
    let mut listener = listen(&a).await;
    let addr = listener.local_addr();
    let serve = tokio::spawn(async move {
        let _ = tokio::time::timeout(Duration::from_secs(5), listener.accept()).await;
    });
    let probed = tokio::time::timeout(Duration::from_secs(4), probe(&b.config, addr))
        .await
        .expect("answered well before the handshake timeout")
        .unwrap();
    assert_eq!(probed.handshake.device, "alpha");
    serve.abort();
}

/// A dialler that pinned the listener but is not pinned back, while pairing is open, must be told
/// no straight away instead of waiting for the pairing code to drop it.
#[tokio::test]
async fn a_dialler_pinned_by_nobody_is_turned_away_at_once_even_while_pairing_is_open() {
    let a = Device::new("alpha", Environment::Production);
    let b = Device::new("bravo", Environment::Production);
    b.pin(&a);
    a.config.trust.set_pairing_open(true);
    let mut listener = listen(&a).await;

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        dial(&b.config, listener.local_addr()),
    )
    .await
    .expect("not left waiting");
    assert!(result.is_err());
    nothing_arrives(&mut listener).await;
}

/// A pairing dialler (nobody pinned) speaks first with something that is not a handshake; the
/// pairing hook still gets that frame.
#[tokio::test]
async fn the_first_pairing_frame_is_not_lost_to_the_stale_peer_check() {
    let a = Device::new("alpha", Environment::Production);
    let b = Device::new("bravo", Environment::Production);
    b.pin(&a);
    a.config.trust.set_pairing_open(true);
    let mut listener = listen(&a).await;
    let addr = listener.local_addr();

    let mut dialler = secure_dial(&b.config, addr).await.unwrap();
    dialler
        .channel
        .send(&json!({"pake": "msg1"}))
        .await
        .unwrap();
    let Incoming::Unpinned(mut unpinned) =
        tokio::time::timeout(Duration::from_secs(5), listener.accept())
            .await
            .unwrap()
            .unwrap()
    else {
        panic!("expected the pairing hook");
    };
    let first: Value = unpinned.channel.recv().await.unwrap();
    assert_eq!(first, json!({"pake": "msg1"}));
}

/// TLS session resumption would skip the certificate check, so a forgotten key could come back
/// on a ticket.
#[tokio::test]
async fn a_forgotten_peer_cannot_resume_an_earlier_tls_session() {
    let (a, b) = pinned_pair(Environment::Production);
    let mut listener = listen(&a).await;
    let addr = listener.local_addr();
    // One client config for both connections, so any ticket it was given is used.
    let connector = tokio_rustls::TlsConnector::from(
        tls::client_config(&b.config.identity, &b.config.trust).unwrap(),
    );
    let connect = || async {
        let tcp = tokio::net::TcpStream::connect(addr)
            .await
            .map_err(|e| e.to_string())?;
        let name = rustls::pki_types::ServerName::try_from(tls::SERVER_NAME).unwrap();
        let stream = connector
            .connect(name, tcp)
            .await
            .map_err(|e| e.to_string())?;
        let mut channel = Channel::new(stream);
        channel
            .send(&b.config.local.get().unwrap())
            .await
            .map_err(|e| e.to_string())?;
        channel.recv::<Handshake>().await.map_err(|e| e.to_string())
    };
    let (first, _accepted) = tokio::join!(connect(), next_peer(&mut listener));
    first.unwrap();

    a.config.trust.unpin(&b.key());
    assert!(
        connect().await.is_err(),
        "no ticket may let a forgotten key back in"
    );
    nothing_arrives(&mut listener).await;
}

#[tokio::test]
async fn at_most_16_inbound_handshakes_are_in_flight() {
    let a = Device::new("alpha", Environment::Production);
    let listener = listen(&a).await;
    let addr = listener.local_addr();
    // Idle TCP connections that never start TLS.
    let mut idle = Vec::new();
    for _ in 0..link::MAX_PENDING {
        idle.push(tokio::net::TcpStream::connect(addr).await.unwrap());
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mut extra = tokio::net::TcpStream::connect(addr).await.unwrap();
    let mut byte = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(2), extra.read(&mut byte))
        .await
        .expect("the surplus connection is dropped, not held");
    assert!(matches!(read, Ok(0) | Err(_)));

    // A slot frees up when one of the idle ones goes away.
    idle.pop();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mut again = tokio::net::TcpStream::connect(addr).await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(300), again.read(&mut byte))
            .await
            .is_err(),
        "held open again"
    );
}

#[tokio::test]
async fn dropping_the_listener_closes_the_port_and_its_pending_connections() {
    let a = Device::new("alpha", Environment::Production);
    let listener = listen(&a).await;
    let addr = listener.local_addr();
    let mut pending = tokio::net::TcpStream::connect(addr).await.unwrap();
    drop(listener);
    let mut byte = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(2), pending.read(&mut byte))
        .await
        .expect("the pending connection is closed with the listener");
    assert!(matches!(read, Ok(0) | Err(_)));
    assert!(tokio::net::TcpStream::connect(addr).await.is_err());
}

#[tokio::test]
async fn dropping_a_transport_ends_the_other_side() {
    let (a, b) = pinned_pair(Environment::Production);
    let mut listener = listen(&a).await;
    let (dialled, accepted) = tokio::join!(
        dial(&b.config, listener.local_addr()),
        next_peer(&mut listener)
    );
    let mut theirs = dialled.unwrap().transport;
    drop(accepted);
    let outcome = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match theirs.try_recv() {
                Ok(None) => tokio::time::sleep(Duration::from_millis(5)).await,
                other => return other,
            }
        }
    })
    .await
    .expect("the line closes");
    assert!(matches!(outcome, Err(SyncError::Disconnected)));
}

#[tokio::test]
async fn listening_on_all_interfaces_is_reachable_by_loopback() {
    let (a, b) = pinned_pair(Environment::Production);
    let mut listener = Listener::bind(a.config.clone(), IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)
        .await
        .unwrap();
    let addr = SocketAddr::new(LOCALHOST, listener.local_addr().port());
    let (dialled, _accepted) = tokio::join!(dial(&b.config, addr), next_peer(&mut listener));
    dialled.unwrap();
}

const LOCALHOST6: IpAddr = IpAddr::V6(std::net::Ipv6Addr::LOCALHOST);

#[tokio::test]
async fn an_ipv6_listener_is_reachable_on_the_ipv6_loopback() {
    let (a, b) = pinned_pair(Environment::Production);
    let mut listener = Listener::bind(a.config.clone(), LOCALHOST6, 0)
        .await
        .unwrap();
    let addr = listener.local_addr();
    assert!(addr.is_ipv6());
    let (dialled, _accepted) = tokio::join!(dial(&b.config, addr), next_peer(&mut listener));
    dialled.unwrap();
}

#[tokio::test]
async fn the_ipv4_and_ipv6_listeners_share_one_port() {
    let (a, b) = pinned_pair(Environment::Production);
    let mut v4 = listen(&a).await;
    let port = v4.local_addr().port();
    let mut v6 = Listener::bind_exact(a.config.clone(), LOCALHOST6, port)
        .await
        .unwrap();
    assert_eq!(v6.local_addr().port(), port);
    let (dialled, _accepted) = tokio::join!(
        dial(&b.config, SocketAddr::new(LOCALHOST6, port)),
        next_peer(&mut v6)
    );
    dialled.unwrap();
    let (dialled, _accepted) = tokio::join!(
        dial(&b.config, SocketAddr::new(LOCALHOST, port)),
        next_peer(&mut v4)
    );
    dialled.unwrap();
}

#[tokio::test]
async fn a_handshake_frame_far_below_8_mb_is_the_most_a_stranger_may_send() {
    let (a, b) = pinned_pair(Environment::Production);
    let mut listener = listen(&a).await;
    let addr = listener.local_addr();
    let channel = secure_dial(&b.config, addr).await.unwrap().channel;
    let header = u32::try_from(link::HANDSHAKE_MAX_FRAME + 1)
        .unwrap()
        .to_be_bytes();
    let mut stream = channel.into_inner();
    stream.write_all(&header).await.unwrap();
    stream.flush().await.unwrap();
    nothing_arrives(&mut listener).await;
    // Our handshake may still arrive first; then the line must end.
    let ended = tokio::time::timeout(Duration::from_secs(2), async {
        let mut sink = Vec::new();
        let _ = stream.read_to_end(&mut sink).await;
    })
    .await;
    assert!(ended.is_ok(), "closed");
}
