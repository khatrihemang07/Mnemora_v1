//! What Sync runs while it is on: the TLS listener, the discovery announcement and pairing. One
//! [`Network`] exists per "Sync on"; dropping it closes the port, stops announcing and ends every
//! task, which is how "inert when off" holds.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use crate::SyncError;
use crate::discovery::Discovery;
use crate::net::{DEFAULT_PORT, DeviceIdentity, Incoming, Listener, NetConfig, Trust};
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
}

impl NetworkOptions {
    /// The app's: every interface (Wi-Fi and Tailscale), port 47311, the real timings.
    pub fn app(discovery: Arc<dyn Discovery>) -> Self {
        Self {
            ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: DEFAULT_PORT,
            timing: Timing::default(),
            discovery,
        }
    }
}

/// The running network side of Sync.
pub struct Network {
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
        host: Arc<dyn PairingHost>,
        identity: Arc<DeviceIdentity>,
        options: NetworkOptions,
    ) -> Result<Self, SyncError> {
        let trust = Trust::new();
        for (key, device) in host.pinned() {
            trust.pin(key, &device);
        }
        let config = NetConfig {
            identity,
            trust: trust.clone(),
            local: host.handshake()?,
        };
        let mut listener = Listener::bind(config.clone(), options.ip, options.port).await?;
        let addr = listener.local_addr();
        let fell_back = listener.fell_back();
        let pairing = Pairing::new(
            host,
            Arc::clone(&options.discovery),
            config,
            addr.port(),
            options.timing,
            Handle::current(),
        );
        let hub = Arc::clone(&pairing);
        let accept = tokio::spawn(async move {
            while let Some(incoming) = listener.accept().await {
                match incoming {
                    Incoming::Unpinned(secured) => hub.handle_incoming(secured),
                    // Sessions with paired devices are wired in by the connection manager.
                    Incoming::Peer(_) => {
                        tracing::debug!(target: "sync", "a paired device connected");
                    }
                }
            }
        });
        pairing.refresh_advert();
        Ok(Self {
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

    /// Live tasks: the accept loop and pairing's.
    pub fn tasks(&self) -> usize {
        usize::from(!self.accept.is_finished()) + self.pairing.tasks()
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
