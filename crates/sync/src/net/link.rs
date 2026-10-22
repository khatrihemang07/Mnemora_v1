//! Sockets: the listener, the dialler, and [`NetTransport`], which makes a connection look like
//! the non-blocking [`Transport`] the engine drives. The engine never awaits (a `TestApp` is not
//! `Send`), so each connection gets two tasks that move frames between the socket and a pair of
//! queues; `send` and `try_recv` only touch the queues.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Duration;

use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::{AbortHandle, JoinHandle};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};

use super::NetError;
use super::frame::{Channel, read_frame, write_frame};
use super::handshake::{Handshake, check};
use super::identity::{DeviceIdentity, PublicKey};
use super::tls::{SERVER_NAME, Trust, client_config, public_key_of, server_config};
use crate::error::SyncError;
use crate::transport::Transport;
use crate::wire::Message;

/// The port Mnemora listens on unless it is taken.
pub const DEFAULT_PORT: u16 = 47311;

/// How long a TCP connect (and each handshake step) may take.
const STEP_TIMEOUT: Duration = Duration::from_secs(3);
/// How long the whole TLS + Mnemora handshake may take.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// A TLS connection carrying framed messages.
pub type TlsChannel = Channel<TlsStream<TcpStream>>;

/// Everything a connection needs: who we are, who we trust, and what we say first.
#[derive(Clone)]
pub struct NetConfig {
    /// This device's key.
    pub identity: Arc<DeviceIdentity>,
    /// The keys we accept.
    pub trust: Trust,
    /// Our handshake.
    pub local: Handshake,
}

/// A TLS connection to a peer whose key has been verified against the [`Trust`] set (pinned, or
/// admitted because pairing is open) and which has said nothing else yet.
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

/// Connects and completes TLS, without any handshake. The peer is pinned or, with pairing open,
/// merely admitted: check [`Secured::pinned`].
pub async fn secure_dial(config: &NetConfig, addr: SocketAddr) -> Result<Secured, NetError> {
    let tls = TlsConnector::from(client_config(&config.identity, &config.trust)?);
    let tcp = tokio::time::timeout(STEP_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| NetError::Timeout)??;
    tcp.set_nodelay(true)?;
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

/// Runs the handshake over a secured connection and starts its transport.
async fn establish(config: &NetConfig, mut secured: Secured) -> Result<Established, NetError> {
    if !secured.pinned {
        return Err(NetError::Refused(super::Refusal::Unpinned));
    }
    let exchange = async {
        secured.channel.send(&config.local).await?;
        secured.channel.recv::<Handshake>().await
    };
    let remote = tokio::time::timeout(HANDSHAKE_TIMEOUT, exchange)
        .await
        .map_err(|_| NetError::Timeout)?
        .map_err(|e| match e {
            NetError::Io(io) => tls_error(io),
            other => other,
        })?;
    let mismatch =
        check(&config.local, &remote, &secured.key, &config.trust).map_err(NetError::Refused)?;
    Ok(Established {
        peer: PeerInfo {
            key: secured.key,
            handshake: remote,
            data_version_mismatch: mismatch,
            remote: secured.remote,
        },
        transport: NetTransport::start(secured.channel.into_inner()),
    })
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
    pub async fn bind(
        config: NetConfig,
        ip: IpAddr,
        preferred_port: u16,
    ) -> Result<Self, NetError> {
        let acceptor = TlsAcceptor::from(server_config(&config.identity, &config.trust)?);
        let (socket, fell_back) = match TcpListener::bind((ip, preferred_port)).await {
            Ok(socket) => (socket, false),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                (TcpListener::bind((ip, 0)).await?, true)
            }
            Err(e) => return Err(e.into()),
        };
        let addr = socket.local_addr()?;
        tracing::info!(target: "sync", port = addr.port(), fell_back, "listening");
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
        let (acceptor, config, out) = (acceptor.clone(), config.clone(), out.clone());
        tokio::spawn(async move {
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
    let stream = tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(tcp))
        .await
        .map_err(|_| NetError::Timeout)?
        .map_err(tls_error)?;
    let secured = secured(TlsStream::Server(stream), &config.trust, remote)?;
    if secured.pinned {
        Ok(Incoming::Peer(Box::new(establish(config, secured).await?)))
    } else {
        Ok(Incoming::Unpinned(Box::new(secured)))
    }
}

/// One end of a network connection as a [`Transport`]. Dropping it closes the connection after
/// what was already sent has been written.
pub struct NetTransport {
    out: UnboundedSender<Message>,
    inbox: Receiver<Message>,
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
        let (to_inbox, inbox) = channel::<Message>();
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
                if let Err(e) = write_frame(&mut write, &message).await {
                    tracing::debug!(target: "sync", error = %e, "write failed");
                    return;
                }
            }
            // The transport was dropped: say goodbye properly.
            let _ = write.shutdown().await;
        });
        Self {
            out,
            inbox,
            reader: reader.abort_handle(),
        }
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
