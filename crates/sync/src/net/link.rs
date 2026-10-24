//! Sockets: the listener, the dialler, and [`NetTransport`], which makes a connection look like
//! the non-blocking [`Transport`] the engine drives. The engine never awaits (a `TestApp` is not
//! `Send`), so each connection gets two tasks that move frames between the socket and a pair of
//! queues; `send` and `try_recv` only touch the queues.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::{AbortHandle, JoinHandle, JoinSet};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};

use super::NetError;
use super::frame::{Channel, read_frame, write_frame};
use super::handshake::{Handshake, check};
use super::identity::{DeviceIdentity, PublicKey};
use super::tls::{SERVER_NAME, Trust, client_config, public_key_of, server_config};
use crate::error::SyncError;
use crate::pause::{Pause, pause_of};
use crate::transport::Transport;
use crate::wire::Message;

/// The port Mnemora listens on unless it is taken.
pub const DEFAULT_PORT: u16 = 47311;

/// How long a TCP connect (and each handshake step) may take.
const STEP_TIMEOUT: Duration = Duration::from_secs(3);
/// How long one frame may take to write before the connection is given up on.
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the whole TLS + Mnemora handshake may take.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// The most inbound connections that may be between accept and "ready" at once. More are
/// closed on arrival, so a flood of idle sockets cannot hold unbounded memory and tasks.
pub const MAX_PENDING: usize = 16;
/// The largest frame accepted before the peer is known to be a paired device (its handshake, or
/// the first pairing message). Far below [`MAX_FRAME`](super::MAX_FRAME).
pub const HANDSHAKE_MAX_FRAME: usize = 64 * 1024;

/// A TLS connection carrying framed messages.
pub type TlsChannel = Channel<TlsStream<TcpStream>>;

/// Builds this device's handshake for each connection, so the group and the addresses it says are
/// the current ones, not those of the moment the port was bound.
#[derive(Clone)]
pub struct LocalHandshake(Arc<dyn Fn() -> Result<Handshake, SyncError> + Send + Sync>);

impl LocalHandshake {
    /// A handshake built by `build` each time one is needed.
    pub fn new(build: impl Fn() -> Result<Handshake, SyncError> + Send + Sync + 'static) -> Self {
        Self(Arc::new(build))
    }

    /// Always the same handshake.
    pub fn fixed(handshake: Handshake) -> Self {
        Self::new(move || Ok(handshake.clone()))
    }

    /// The handshake to send now. May read the database: call it off the async threads.
    pub fn get(&self) -> Result<Handshake, SyncError> {
        (self.0)()
    }
}

/// Everything a connection needs: who we are, who we trust, and what we say first.
#[derive(Clone)]
pub struct NetConfig {
    /// This device's key.
    pub identity: Arc<DeviceIdentity>,
    /// The keys we accept.
    pub trust: Trust,
    /// Our handshake, built per connection.
    pub local: LocalHandshake,
}

/// A TLS connection to a peer whose key has been verified against the [`Trust`] set (pinned, or
/// admitted because pairing is open). From [`Listener`] an unpinned one has already sent its
/// first frame, which `recv` still returns (it is never a [`Handshake`]).
pub struct Secured {
    /// The connection.
    pub channel: TlsChannel,
    /// The peer's public key, proven by the TLS handshake.
    pub key: PublicKey,
    /// Whether the key is pinned.
    pub pinned: bool,
    /// The peer's socket address.
    pub remote: SocketAddr,
}

/// What a peer's handshake told us.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    /// The peer's key.
    pub key: PublicKey,
    /// Its handshake.
    pub handshake: Handshake,
    /// Modules whose data versions differ from ours (see `Handshake::data_versions`); the
    /// session must not run while this is non-empty (#72).
    pub data_version_mismatch: Vec<String>,
    /// Why no session may run with it (data versions or clocks); `None` when it may.
    pub pause: Option<Pause>,
    /// Its socket address.
    pub remote: SocketAddr,
}

/// A connection ready for a session.
pub struct Established {
    /// To hand to an `Endpoint`.
    pub transport: NetTransport,
    /// Who is on the other end.
    pub peer: PeerInfo,
}

/// What the listener hands out.
pub enum Incoming {
    /// A pinned peer that passed the handshake.
    Peer(Box<Established>),
    /// A peer with an unpinned key, admitted by TLS only because pairing is open. For #62.
    Unpinned(Box<Secured>),
}

fn tls_error(e: std::io::Error) -> NetError {
    match e.get_ref().and_then(|i| i.downcast_ref::<rustls::Error>()) {
        Some(inner) => NetError::Tls(inner.to_string()),
        None => NetError::Io(e),
    }
}

/// Whether the other end turned this device away at the TLS handshake because it takes no
/// unpinned key (no code is open there). Under TLS 1.3 that only surfaces on the first read.
pub(crate) fn is_denied(e: &NetError) -> bool {
    let denied = |inner: &rustls::Error| {
        matches!(
            inner,
            rustls::Error::AlertReceived(rustls::AlertDescription::AccessDenied)
        )
    };
    match e {
        NetError::Io(io) => io
            .get_ref()
            .and_then(|i| i.downcast_ref::<rustls::Error>())
            .is_some_and(denied),
        NetError::Tls(text) => text.contains("AccessDenied"),
        _ => false,
    }
}

fn peer_key<S>(conn: &tokio_rustls::TlsStream<S>) -> Result<PublicKey, NetError> {
    let certs = match conn {
        TlsStream::Client(c) => c.get_ref().1.peer_certificates(),
        TlsStream::Server(s) => s.get_ref().1.peer_certificates(),
    };
    certs
        .and_then(|c| c.first())
        .and_then(public_key_of)
        .ok_or_else(|| NetError::Tls("the peer sent no usable certificate".into()))
}

fn secured(
    stream: TlsStream<TcpStream>,
    trust: &Trust,
    remote: SocketAddr,
) -> Result<Secured, NetError> {
    let key = peer_key(&stream)?;
    Ok(Secured {
        pinned: trust.is_pinned(&key),
        channel: Channel::new(stream),
        key,
        remote,
    })
}

/// Idle time before the first keepalive probe, the gap between probes and how many go unanswered
/// before the line is given up on: a device that vanished without closing (Wi-Fi off, out of
/// range) makes the connection fail about 14 s after its last word, so the row reads "Not nearby"
/// quickly instead of after the 60 s a session may stay silent.
const KEEPALIVE_IDLE: Duration = Duration::from_secs(5);
const KEEPALIVE_EVERY: Duration = Duration::from_secs(3);
const KEEPALIVE_PROBES: u32 = 3;

/// Turns on TCP keepalive for a connection. A socket that refuses it is simply left as it is.
fn keep_alive(tcp: &TcpStream) {
    let probe = socket2::TcpKeepalive::new()
        .with_time(KEEPALIVE_IDLE)
        .with_interval(KEEPALIVE_EVERY);
    // Windows has no probe count to set.
    #[cfg(not(windows))]
    let probe = probe.with_retries(KEEPALIVE_PROBES);
    if let Err(e) = socket2::SockRef::from(tcp).set_tcp_keepalive(&probe) {
        tracing::debug!(target: "sync", error = %e, "could not turn on keepalive");
    }
}

/// Connects and completes TLS, without any handshake. The peer is pinned or, with pairing open,
/// merely admitted: check [`Secured::pinned`].
pub async fn secure_dial(config: &NetConfig, addr: SocketAddr) -> Result<Secured, NetError> {
    let tls = TlsConnector::from(client_config(&config.identity, &config.trust)?);
    let tcp = tokio::time::timeout(STEP_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| NetError::Timeout)??;
    tcp.set_nodelay(true)?;
    keep_alive(&tcp);
    let name = ServerName::try_from(SERVER_NAME).map_err(|e| NetError::Tls(e.to_string()))?;
    let stream = tokio::time::timeout(HANDSHAKE_TIMEOUT, tls.connect(name, tcp))
        .await
        .map_err(|_| NetError::Timeout)?
        .map_err(tls_error)?;
    secured(TlsStream::Client(stream), &config.trust, addr)
}

/// Dials `addr` and, if its key is pinned and the handshake passes, returns the connection.
/// Under TLS 1.3 a server that rejects our certificate is only noticed on the first read, so
/// that failure also surfaces from here, as [`NetError::Tls`].
pub async fn dial(config: &NetConfig, addr: SocketAddr) -> Result<Established, NetError> {
    let secured = secure_dial(config, addr).await?;
    establish(config, secured).await
}

/// Who answered a [`probe`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probed {
    /// The key it proved.
    pub key: PublicKey,
    /// What it says about itself.
    pub handshake: Handshake,
}

/// Connects to `addr` and reads who is there, without taking part in a session: TLS (so only a
/// device whose key is pinned answers, and only if it still trusts ours), then the other end's
/// handshake. Ours is never sent, so the other end ends the attempt at once and starts nothing.
/// For "Check" in the device sheet.
pub async fn probe(config: &NetConfig, addr: SocketAddr) -> Result<Probed, NetError> {
    let mut secured = secure_dial(config, addr).await?;
    if !secured.pinned {
        return Err(NetError::Refused(super::Refusal::Unpinned));
    }
    let handshake = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        secured.channel.recv_max::<Handshake>(HANDSHAKE_MAX_FRAME),
    )
    .await
    .map_err(|_| NetError::Timeout)?
    .map_err(|e| match e {
        NetError::Io(io) => tls_error(io),
        other => other,
    })?;
    Ok(Probed {
        key: secured.key,
        handshake,
    })
}

/// Runs the handshake over a secured connection and starts its transport.
async fn establish(config: &NetConfig, mut secured: Secured) -> Result<Established, NetError> {
    if !secured.pinned {
        return Err(NetError::Refused(super::Refusal::Unpinned));
    }
    let local = {
        let build = config.local.clone();
        tokio::task::spawn_blocking(move || build.get())
            .await
            .map_err(|e| NetError::Local(e.to_string()))?
            .map_err(|e| NetError::Local(e.to_string()))?
    };
    let exchange = async {
        secured.channel.send(&local).await?;
        secured
            .channel
            .recv_max::<Handshake>(HANDSHAKE_MAX_FRAME)
            .await
    };
    let remote = tokio::time::timeout(HANDSHAKE_TIMEOUT, exchange)
        .await
        .map_err(|_| NetError::Timeout)?
        .map_err(|e| match e {
            NetError::Io(io) => tls_error(io),
            other => other,
        })?;
    let mismatch =
        check(&local, &remote, &secured.key, &config.trust).map_err(NetError::Refused)?;
    let pause = pause_of(&local, &remote);
    Ok(Established {
        peer: PeerInfo {
            key: secured.key,
            handshake: remote,
            data_version_mismatch: mismatch,
            pause,
            remote: secured.remote,
        },
        transport: NetTransport::start(secured.channel.into_inner()),
    })
}

/// A listening socket on `ip:port`. An IPv6 address is bound IPv6-only, so a `::` listener can
/// share its port with the `0.0.0.0` one on every platform (dual-stack is the default on some and
/// not on others).
fn open_socket(ip: IpAddr, port: u16) -> std::io::Result<TcpListener> {
    use socket2::{Domain, Socket, Type};
    let addr = SocketAddr::new(ip, port);
    let socket = Socket::new(Domain::for_address(addr), Type::STREAM, None)?;
    if ip.is_ipv6() {
        socket.set_only_v6(true)?;
    }
    #[cfg(not(windows))]
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    TcpListener::from_std(socket.into())
}

/// Accepts connections on one port.
pub struct Listener {
    addr: SocketAddr,
    fell_back: bool,
    incoming: UnboundedReceiver<Incoming>,
    task: JoinHandle<()>,
}

impl Listener {
    /// Listens on `ip:preferred_port`, or on an ephemeral port of `ip` if that one is busy.
    /// Pass [`DEFAULT_PORT`] in the app; tests pass a port of their own so a busy 47311 on the
    /// machine cannot fail them.
    #[allow(
        clippy::unused_async,
        reason = "async so it can only be called inside the Tokio runtime the socket registers with"
    )]
    pub async fn bind(
        config: NetConfig,
        ip: IpAddr,
        preferred_port: u16,
    ) -> Result<Self, NetError> {
        let (socket, fell_back) = match open_socket(ip, preferred_port) {
            Ok(socket) => (socket, false),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => (open_socket(ip, 0)?, true),
            Err(e) => return Err(e.into()),
        };
        Self::serve(config, socket, fell_back)
    }

    /// Listens on exactly `ip:port` (no fallback): the second listener of a device, which must
    /// share the first one's port because that is the one advertised.
    #[allow(
        clippy::unused_async,
        reason = "async so it can only be called inside the Tokio runtime the socket registers with"
    )]
    pub async fn bind_exact(config: NetConfig, ip: IpAddr, port: u16) -> Result<Self, NetError> {
        Self::serve(config, open_socket(ip, port)?, false)
    }

    fn serve(config: NetConfig, socket: TcpListener, fell_back: bool) -> Result<Self, NetError> {
        let acceptor = TlsAcceptor::from(server_config(&config.identity, &config.trust)?);
        let addr = socket.local_addr()?;
        tracing::info!(target: "sync", port = addr.port(), fell_back, v6 = addr.is_ipv6(), "listening");
        let (tx, incoming) = unbounded_channel();
        let task = tokio::spawn(accept_loop(socket, acceptor, config, tx));
        Ok(Self {
            addr,
            fell_back,
            incoming,
            task,
        })
    }

    /// The address actually bound.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// The preferred port was busy, so an ephemeral one is used.
    pub fn fell_back(&self) -> bool {
        self.fell_back
    }

    /// The next connection that passed TLS (and, if pinned, the handshake). Peers that fail are
    /// dropped and never appear here.
    pub async fn accept(&mut self) -> Option<Incoming> {
        self.incoming.recv().await
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn accept_loop(
    socket: TcpListener,
    acceptor: TlsAcceptor,
    config: NetConfig,
    out: UnboundedSender<Incoming>,
) {
    // Owning the connection tasks here means aborting this task (the Listener dropped) also
    // ends every connection still being set up.
    let mut pending = JoinSet::new();
    loop {
        let (tcp, remote) = match socket.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                // Usually a transient resource limit: back off rather than spin.
                tracing::debug!(target: "sync", error = %e, "accept failed");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        while pending.try_join_next().is_some() {}
        if pending.len() >= MAX_PENDING {
            tracing::debug!(target: "sync", "too many connections being set up, closing one");
            continue;
        }
        let (acceptor, config, out) = (acceptor.clone(), config.clone(), out.clone());
        pending.spawn(async move {
            match serve(tcp, remote, acceptor, &config).await {
                Ok(incoming) => {
                    let _ = out.send(incoming);
                }
                Err(e) => {
                    tracing::debug!(target: "sync", error = %e, "refused an incoming connection");
                }
            }
        });
    }
}

async fn serve(
    tcp: TcpStream,
    remote: SocketAddr,
    acceptor: TlsAcceptor,
    config: &NetConfig,
) -> Result<Incoming, NetError> {
    tcp.set_nodelay(true)?;
    keep_alive(&tcp);
    let stream = tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(tcp))
        .await
        .map_err(|_| NetError::Timeout)?
        .map_err(tls_error)?;
    let mut secured = secured(TlsStream::Server(stream), &config.trust, remote)?;
    // Admitted only because pairing was open, and it may have closed since.
    if !secured.pinned && !config.trust.pairing_open() {
        return Err(NetError::Refused(super::Refusal::Unpinned));
    }
    // A device whose key is pinned here may still be pairing: it forgot this device while this
    // one did not forget it, and now types the code shown. A peer sends its handshake first, a
    // pairing device its own first message, so while pairing is open the first frame says which.
    if secured.pinned && !config.trust.pairing_open() {
        return Ok(Incoming::Peer(Box::new(establish(config, secured).await?)));
    }
    // A device that pinned us but that we no longer know (forgotten here) dials as a normal
    // peer and sends its handshake first. Turn it away now instead of leaving it waiting on the
    // pairing code; a pairing dialler's first frame is anything else and is kept for the hook.
    let first = tokio::time::timeout(HANDSHAKE_TIMEOUT, secured.channel.peek(HANDSHAKE_MAX_FRAME))
        .await
        .map_err(|_| NetError::Timeout)??;
    if serde_json::from_value::<Handshake>(first.clone()).is_ok() {
        if secured.pinned {
            return Ok(Incoming::Peer(Box::new(establish(config, secured).await?)));
        }
        return Err(NetError::Refused(super::Refusal::Unpinned));
    }
    Ok(Incoming::Unpinned(Box::new(secured)))
}

/// One end of a network connection as a [`Transport`]. Dropping it closes the connection after
/// what was already sent has been written.
pub struct NetTransport {
    out: UnboundedSender<Message>,
    inbox: UnboundedReceiver<Message>,
    reader: AbortHandle,
}

impl NetTransport {
    /// Starts the reader and writer tasks for `stream`.
    fn start<S>(stream: S) -> Self
    where
        S: AsyncRead + AsyncWrite + Send + 'static,
    {
        let (mut read, mut write) = tokio::io::split(stream);
        let (out, mut queued) = unbounded_channel::<Message>();
        let (to_inbox, inbox) = unbounded_channel::<Message>();
        let reader = tokio::spawn(async move {
            loop {
                match read_frame::<_, Message>(&mut read).await {
                    Ok(message) => {
                        if to_inbox.send(message).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        // Closing the inbox is how the engine learns the line is gone; anything
                        // already received is still delivered first.
                        tracing::debug!(target: "sync", error = %e, "connection ended");
                        break;
                    }
                }
            }
        });
        tokio::spawn(async move {
            while let Some(message) = queued.recv().await {
                // A peer that stops reading must not pin this task (and the socket) forever.
                match tokio::time::timeout(WRITE_TIMEOUT, write_frame(&mut write, &message)).await {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        tracing::debug!(target: "sync", error = %e, "write failed");
                        return;
                    }
                    Err(_) => {
                        tracing::debug!(target: "sync", "write timed out");
                        return;
                    }
                }
            }
            // The transport was dropped: say goodbye properly.
            let _ = tokio::time::timeout(STEP_TIMEOUT, write.shutdown()).await;
        });
        Self {
            out,
            inbox,
            reader: reader.abort_handle(),
        }
    }
}

impl NetTransport {
    /// Waits for the next message; fails with [`SyncError::Disconnected`] once the connection is
    /// gone and everything received has been taken.
    pub async fn recv(&mut self) -> Result<Message, SyncError> {
        self.inbox.recv().await.ok_or(SyncError::Disconnected)
    }
}

impl Drop for NetTransport {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

impl Transport for NetTransport {
    fn send(&mut self, message: Message) -> Result<(), SyncError> {
        self.out.send(message).map_err(|_| SyncError::Disconnected)
    }

    fn try_recv(&mut self) -> Result<Option<Message>, SyncError> {
        match self.inbox.try_recv() {
            Ok(message) => Ok(Some(message)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(SyncError::Disconnected),
        }
    }
}

#[cfg(test)]
mod keepalive_tests {
    use super::*;

    #[tokio::test]
    async fn a_connection_probes_an_idle_line_so_a_vanished_device_is_noticed_in_seconds() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        keep_alive(&tcp);
        let socket = socket2::SockRef::from(&tcp);
        assert!(socket.keepalive().unwrap());
        assert_eq!(socket.tcp_keepalive_time().unwrap(), KEEPALIVE_IDLE);
        assert_eq!(socket.tcp_keepalive_interval().unwrap(), KEEPALIVE_EVERY);
        #[cfg(not(windows))]
        assert_eq!(socket.tcp_keepalive_retries().unwrap(), KEEPALIVE_PROBES);
        // 5 s idle + 3 unanswered probes 3 s apart: well inside the 15 s the spec asks for.
        let worst = KEEPALIVE_IDLE + KEEPALIVE_EVERY * KEEPALIVE_PROBES;
        assert!(worst <= Duration::from_secs(15));
    }
}
