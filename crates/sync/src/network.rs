//! What Sync runs while it is on: the TLS listener, the discovery announcement, pairing and the
//! session manager. One [`Network`] exists per "Sync on"; dropping it closes the port, stops
//! announcing, ends every connection, dial and timer, which is how "inert when off" holds.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};

use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use crate::SyncError;
use crate::discovery::Discovery;
use crate::live::{Host, Live, Pace, PeerLive};
use crate::net::{
    DEFAULT_PORT, DeviceIdentity, Incoming, Listener, LocalHandshake, NetConfig, Trust,
};
use crate::pair::{Pairing, PairingHost, Timing};

/// How a [`Network`] starts.
pub struct NetworkOptions {
    /// The address to listen on.
    pub ip: IpAddr,
    /// The port to try first; another is used if it is busy.
    pub port: u16,
    /// Pairing timings.
    pub timing: Timing,
    /// Announcing and finding devices.
    pub discovery: Arc<dyn Discovery>,
    /// How eagerly paired devices are synced and dialled.
    pub pace: Pace,
}

impl NetworkOptions {
    /// The app's: every interface (Wi-Fi and Tailscale), port 47311, the real timings.
    pub fn app(discovery: Arc<dyn Discovery>) -> Self {
        Self {
            ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: DEFAULT_PORT,
            timing: Timing::default(),
            discovery,
            pace: Pace::default(),
        }
    }
}

/// The running network side of Sync.
pub struct Network {
    live: Live,
    pairing: Arc<Pairing>,
    addr: SocketAddr,
    fell_back: bool,
    accept: JoinHandle<()>,
    discovery: Arc<dyn Discovery>,
    trust: Trust,
}

impl Network {
    /// Binds the port, pins the paired devices' keys, announces this device and starts accepting.
    /// Call inside the Tokio runtime.
    pub async fn start(
        host: Arc<dyn Host>,
        identity: Arc<DeviceIdentity>,
        options: NetworkOptions,
    ) -> Result<Self, SyncError> {
        let trust = Trust::new();
        for (key, device) in host.pinned() {
            trust.pin(key, &device);
        }
        let me = host.handshake()?.device;
        // The port is only known once bound, and the group and addresses change over time: the
        // handshake is built afresh for every connection.
        let port = Arc::new(AtomicU16::new(0));
        let local = {
            let (host, port) = (Arc::clone(&host), Arc::clone(&port));
            LocalHandshake::new(move || {
                let mut handshake = host.handshake()?;
                let port = port.load(Ordering::SeqCst);
                handshake.addresses = crate::addresses::local()
                    .into_iter()
                    .map(|ip| SocketAddr::new(ip, port).to_string())
                    .collect();
                Ok(handshake)
            })
        };
        let config = NetConfig {
            identity,
            trust: trust.clone(),
            local,
        };
        let mut listener = Listener::bind(config.clone(), options.ip, options.port).await?;
        let addr = listener.local_addr();
        port.store(addr.port(), Ordering::SeqCst);
        let fell_back = listener.fell_back();
        let live = Live::start(
            Arc::clone(&host),
            config.clone(),
            Arc::clone(&options.discovery),
            options.pace,
            me,
            &Handle::current(),
        );
        let pairing_host: Arc<dyn PairingHost> = host;
        let pairing = Pairing::new(
            pairing_host,
            Arc::clone(&options.discovery),
            config,
            addr.port(),
            options.timing,
            Handle::current(),
        );
        let hub = Arc::clone(&pairing);
        let sessions = live.acceptor();
        let accept = tokio::spawn(async move {
            while let Some(incoming) = listener.accept().await {
                match incoming {
                    Incoming::Unpinned(secured) => hub.handle_incoming(secured),
                    Incoming::Peer(established) => sessions.accept(*established),
                }
            }
        });
        pairing.refresh_advert();
        Ok(Self {
            live,
            pairing,
            addr,
            fell_back,
            accept,
            discovery: options.discovery,
            trust,
        })
    }

    /// Pairing: show a code, type one, allow, cancel.
    pub fn pairing(&self) -> &Arc<Pairing> {
        &self.pairing
    }

    /// The address bound (the unspecified address when listening on every interface).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The preferred port was busy, so another is used.
    pub fn fell_back(&self) -> bool {
        self.fell_back
    }

    /// Live announcements and browses.
    pub fn discovery_active(&self) -> usize {
        self.discovery.active()
    }

    /// Live tasks: the accept loop, pairing's and the session manager's.
    pub fn tasks(&self) -> usize {
        usize::from(!self.accept.is_finished()) + self.pairing.tasks() + self.live.tasks()
    }

    /// Sessions started with paired devices.
    pub fn sessions(&self) -> usize {
        self.live.sessions()
    }

    /// The session manager.
    pub fn live(&self) -> &Live {
        &self.live
    }

    /// What is happening with each paired device.
    pub fn peers_live(&self) -> HashMap<String, PeerLive> {
        self.live.snapshot()
    }

    /// Announces again (Visible on this network changed, the group changed).
    pub fn refresh_advert(&self) {
        self.pairing.refresh_advert();
    }

    /// Stops trusting every key (this device left its group).
    pub fn unpin_all(&self) {
        self.trust.unpin_all();
    }
}

impl Drop for Network {
    fn drop(&mut self) {
        self.accept.abort();
        self.pairing.shutdown();
    }
}
