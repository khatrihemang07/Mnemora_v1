//! The supervisor: one Tokio task that owns every dial and every connection of a running Sync,
//! decides who to dial and when, resolves two connections to the same device, and starts
//! sessions when this device has changes to push. Everything it spawns lives in its [`JoinSet`],
//! so aborting the supervisor (dropping [`Live`]) ends it all.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::runtime::Handle;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::{JoinError, JoinHandle, JoinSet};
use tokio::time::{Instant, sleep_until};

use super::backoff::Backoff;
use super::conn::{self, Conn, ConnCmd, RoundDone};
use super::dial::{candidates, race};
use super::shared::Shared;
use super::status::PeerLive;
use super::tie::replaces;
use super::{Host, Pace, blocking};
use crate::discovery::{Browsing, Candidate, Discovery};
use crate::error::SyncError;
use crate::net::{Established, NetConfig};
use crate::peer_state::{self, Route};
use crate::peers;

/// A running Sync's connections. Dropping it stops them all.
pub struct Live {
    cmds: UnboundedSender<Cmd>,
    shared: Arc<Shared>,
    task: JoinHandle<()>,
}

/// Hands the connections other devices open to the supervisor.
#[derive(Clone)]
pub struct Acceptor(UnboundedSender<Cmd>);

impl Acceptor {
    /// A paired device opened a connection to us.
    pub fn accept(&self, established: Established) {
        let _ = self.0.send(Cmd::Accept(Box::new(established)));
    }
}

enum Cmd {
    /// A connection another device opened to us.
    Accept(Box<Established>),
    /// A connection we opened (a test hands one over; dials adopt their own).
    #[cfg(test)]
    Adopt(Box<Established>),
    SyncNow,
    Resume,
    Refresh,
    Reconfigure,
}

impl Live {
    /// Starts the supervisor on `runtime`: it dials the paired devices it knows (if Sync
    /// automatically is on) and watches for them on the network.
    pub fn start(
        host: Arc<dyn Host>,
        config: NetConfig,
        discovery: Arc<dyn Discovery>,
        pace: Pace,
        me: String,
        runtime: &Handle,
    ) -> Self {
        let shared = Arc::new(Shared::new(host, me));
        // Looking and counting start now, not when the task first runs, so what is running is
        // what the caller was told.
        shared.tasks.store(1, Ordering::SeqCst);
        let browse = match discovery.browse() {
            Ok(b) => Some(b),
            Err(e) => {
                tracing::warn!(target: "sync", error = %e, "not looking for devices on the network");
                None
            }
        };
        let (cmds, inbox) = unbounded_channel();
        let (done_tx, rounds) = unbounded_channel();
        let supervisor = Supervisor {
            shared: Arc::clone(&shared),
            config,
            pace,
            peers: HashMap::new(),
            set: JoinSet::new(),
            done_tx,
            next_id: 0,
        };
        let task = runtime.spawn(supervisor.run(inbox, rounds, browse));
        Self { cmds, shared, task }
    }

    /// A paired device opened a connection to us.
    pub fn accept(&self, established: Established) {
        self.acceptor().accept(established);
    }

    /// A cheap handle for the listener's task to hand connections over with.
    pub fn acceptor(&self) -> Acceptor {
        Acceptor(self.cmds.clone())
    }

    /// Hands over a connection this device opened (the supervisor's own dials arrive the same way).
    #[cfg(test)]
    pub(super) fn adopt(&self, established: Established) {
        let _ = self.cmds.send(Cmd::Adopt(Box::new(established)));
    }

    /// "Sync now": sync with every reachable device now, whatever "Sync automatically" says.
    pub fn sync_now(&self) {
        let _ = self.cmds.send(Cmd::SyncNow);
    }

    /// The network changed or the app came back: try unreachable devices again from the start of
    /// the backoff.
    pub fn resume(&self) {
        let _ = self.cmds.send(Cmd::Resume);
    }

    /// The set of paired devices changed.
    pub fn refresh_peers(&self) {
        let _ = self.cmds.send(Cmd::Refresh);
    }

    /// "Sync automatically" was switched.
    pub fn options_changed(&self) {
        let _ = self.cmds.send(Cmd::Reconfigure);
    }

    /// What is happening with each paired device, by device id.
    pub fn snapshot(&self) -> HashMap<String, PeerLive> {
        self.shared.snapshot()
    }

    /// Whether this device dialled the connection it holds with `peer`; `None` without one.
    pub fn dialled_by_me(&self, peer: &str) -> Option<bool> {
        self.shared.dialled(peer)
    }

    #[cfg(test)]
    pub(super) fn host(&self) -> &Arc<dyn Host> {
        &self.shared.host
    }

    /// Live tasks: the supervisor, every dial in flight and every connection.
    pub fn tasks(&self) -> usize {
        if self.task.is_finished() {
            0
        } else {
            self.shared.tasks.load(Ordering::SeqCst)
        }
    }

    /// Sessions started since this began.
    pub fn sessions(&self) -> usize {
        self.shared.sessions.load(Ordering::SeqCst)
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.task.abort();
    }
}

enum Done {
    Dial {
        peer: String,
        established: Option<Established>,
    },
    Conn {
        peer: String,
        id: u64,
        result: Result<(), SyncError>,
    },
}

struct Held {
    id: u64,
    by_me: bool,
    tx: UnboundedSender<ConnCmd>,
}

struct Peer {
    /// Addresses discovery last reported.
    found: Vec<SocketAddr>,
    conn: Option<Held>,
    dialling: bool,
    backoff: Backoff,
    retry_at: Option<Instant>,
    /// "Sync now" asked for this device and has not finished yet.
    manual: bool,
}

struct Supervisor {
    shared: Arc<Shared>,
    config: NetConfig,
    pace: Pace,
    peers: HashMap<String, Peer>,
    set: JoinSet<Done>,
    done_tx: UnboundedSender<RoundDone>,
    next_id: u64,
}

/// The next device discovery found; never ready without a browse.
async fn next_found(browse: &mut Option<Browsing>) -> Option<Candidate> {
    match browse {
        Some(b) => b.found.recv().await,
        None => std::future::pending().await,
    }
}

async fn wait_until(at: Option<Instant>) {
    match at {
        Some(at) => sleep_until(at).await,
        None => std::future::pending().await,
    }
}

impl Supervisor {
    fn host(&self) -> &Arc<dyn Host> {
        &self.shared.host
    }

    fn automatic(&self) -> bool {
        self.host().automatic()
    }

    async fn run(
        mut self,
        mut cmds: UnboundedReceiver<Cmd>,
        mut rounds: UnboundedReceiver<RoundDone>,
        mut browse: Option<Browsing>,
    ) {
        let changes = self.host().local_changes();
        self.reload().await;
        if self.automatic() {
            self.dial_all().await;
        }
        let mut push_at: Option<Instant> = None;
        loop {
            self.shared
                .tasks
                .store(1 + self.set.len(), Ordering::SeqCst);
            let retry_at = self.peers.values().filter_map(|p| p.retry_at).min();
            tokio::select! {
                Some(cmd) = cmds.recv() => self.command(cmd).await,
                Some(round) = rounds.recv() => self.round_done(&round),
                Some(done) = self.set.join_next() => self.finished(done).await,
                () = changes.notified() => push_at = Some(Instant::now() + self.pace.debounce),
                found = next_found(&mut browse) => match found {
                    Some(candidate) => self.discovered(candidate).await,
                    None => browse = None,
                },
                () = wait_until(push_at) => {
                    push_at = None;
                    self.push();
                }
                () = wait_until(retry_at) => self.retry().await,
            }
        }
    }

    async fn command(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Accept(established) => self.adopt(*established, false).await,
            #[cfg(test)]
            Cmd::Adopt(established) => self.adopt(*established, true).await,
            Cmd::SyncNow => self.sync_now().await,
            Cmd::Resume => self.resume().await,
            Cmd::Refresh => {
                self.reload().await;
                if self.automatic() {
                    self.dial_all().await;
                }
            }
            Cmd::Reconfigure => self.reconfigure().await,
        }
    }

    /// Reads the paired devices: new ones are tracked, forgotten ones dropped.
    async fn reload(&mut self) {
        let ids = blocking(self.host(), |conn| {
            Ok(peers::list(conn)?
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<_>>())
        })
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(target: "sync", error = %e, "could not read the paired devices");
            Vec::new()
        });
        for id in &ids {
            self.peers.entry(id.clone()).or_insert_with(|| Peer {
                found: Vec::new(),
                conn: None,
                dialling: false,
                backoff: Backoff::new(self.pace.retry.clone()),
                retry_at: None,
                manual: false,
            });
        }
        let gone: Vec<String> = self
            .peers
            .keys()
            .filter(|id| !ids.contains(id))
            .cloned()
            .collect();
        for id in gone {
            // Dropping the entry drops the connection's command channel, which closes it.
            self.peers.remove(&id);
            self.shared.remove(&id);
        }
    }

    async fn dial_all(&mut self) {
        let ids: Vec<String> = self
            .peers
            .iter()
            .filter(|(_, p)| p.conn.is_none() && !p.dialling)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.dial(&id).await;
        }
    }

    /// Starts racing the addresses known for `id`, unless it is connected or already being dialled.
    async fn dial(&mut self, id: &str) {
        let Some(peer) = self.peers.get_mut(id) else {
            return;
        };
        if peer.conn.is_some() || peer.dialling {
            return;
        }
        peer.dialling = true;
        peer.retry_at = None;
        let found = peer.found.clone();
        let device = id.to_owned();
        let remembered = blocking(self.host(), {
            let device = device.clone();
            move |conn| Ok(peer_state::addresses_of(conn, &device)?)
        })
        .await
        .unwrap_or_default();
        // The typed Address (#71) joins here as a third source.
        let addrs = candidates(&found, &remembered, &[]);
        let (config, each) = (self.config.clone(), self.pace.dial_each);
        self.set.spawn(async move {
            let established = if addrs.is_empty() {
                None
            } else {
                race(&config, addrs, each).await
            };
            Done::Dial {
                peer: device,
                established,
            }
        });
    }

    fn dial_failed(&mut self, id: &str) {
        let auto = self.automatic();
        let Some(peer) = self.peers.get_mut(id) else {
            return;
        };
        peer.dialling = false;
        if auto {
            peer.retry_at = Some(Instant::now() + peer.backoff.next_delay());
        } else {
            // "Sync now" tried once; nothing keeps trying while Sync automatically is off.
            peer.manual = false;
            peer.retry_at = None;
        }
        self.shared.update(id, true, |l| {
            l.reachable = false;
            l.syncing = false;
            l.progress = None;
        });
    }

    async fn retry(&mut self) {
        let now = Instant::now();
        let due: Vec<String> = self
            .peers
            .iter()
            .filter(|(_, p)| p.retry_at.is_some_and(|at| at <= now))
            .map(|(id, _)| id.clone())
            .collect();
        for id in due {
            self.dial(&id).await;
        }
    }

    async fn finished(&mut self, done: Result<Done, JoinError>) {
        let Ok(done) = done else {
            return;
        };
        match done {
            Done::Dial { peer, established } => {
                if let Some(p) = self.peers.get_mut(&peer) {
                    p.dialling = false;
                }
                match established {
                    Some(established) => {
                        let reached = established.peer.handshake.device.clone();
                        if reached != peer {
                            self.dial_failed(&peer);
                        }
                        self.adopt(established, true).await;
                    }
                    None => self.dial_failed(&peer),
                }
            }
            Done::Conn { peer, id, result } => self.connection_ended(&peer, id, result),
        }
    }

    fn connection_ended(&mut self, id_of_peer: &str, id: u64, result: Result<(), SyncError>) {
        let auto = self.automatic();
        let Some(peer) = self.peers.get_mut(id_of_peer) else {
            return;
        };
        if peer.conn.as_ref().is_none_or(|c| c.id != id) {
            // A connection that was replaced; the one that replaced it carries on.
            return;
        }
        peer.conn = None;
        self.shared.set_dialled(id_of_peer, None);
        match result {
            Ok(()) => {
                // Closed on purpose (a "Sync now" with Sync automatically off, or it was switched
                // off): the device was reached and is still thought of as reachable.
                peer.manual = false;
            }
            Err(e) => {
                tracing::debug!(target: "sync", error = %e, "connection ended");
                if auto {
                    peer.retry_at = Some(Instant::now() + peer.backoff.next_delay());
                }
                peer.manual = false;
                self.shared.update(id_of_peer, true, |l| {
                    l.reachable = false;
                    l.syncing = false;
                    l.progress = None;
                });
            }
        }
    }

    fn round_done(&mut self, round: &RoundDone) {
        if let Some(peer) = self.peers.get_mut(&round.peer) {
            peer.backoff.reset();
            peer.manual = false;
        }
    }

    /// Takes a connection that has passed the handshake. `by_me`: this device dialled it.
    async fn adopt(&mut self, established: Established, by_me: bool) {
        let id = established.peer.handshake.device.clone();
        if !self.peers.contains_key(&id) {
            // Paired a moment ago, perhaps: look again before turning it away.
            self.reload().await;
        }
        let automatic = self.automatic();
        let me = self.shared.me.clone();
        let Some(peer) = self.peers.get_mut(&id) else {
            tracing::debug!(target: "sync", "a connection from a device that is not paired, closed");
            return;
        };
        if !established.peer.data_version_mismatch.is_empty() {
            // The other device's data has another shape (#72 explains it to the person): no
            // session, and the usual backoff so neither side hammers the other.
            tracing::info!(target: "sync", modules = ?established.peer.data_version_mismatch, "paused: data versions differ");
            if automatic && by_me {
                peer.retry_at = Some(Instant::now() + peer.backoff.next_delay());
            }
            self.shared.update(&id, true, |l| {
                l.paused = true;
                l.syncing = false;
                l.progress = None;
            });
            return;
        }
        if !by_me && !automatic && !peer.manual {
            tracing::debug!(target: "sync", "Sync automatically is off, an incoming connection was closed");
            return;
        }
        if let Some(held) = &peer.conn {
            if !replaces(&me, &id, held.by_me, by_me) {
                tracing::debug!(target: "sync", "keeping the connection dialled by the lower id");
                return;
            }
            let _ = held.tx.send(ConnCmd::Close);
        }
        self.next_id += 1;
        let conn_id = self.next_id;
        let (tx, cmds) = unbounded_channel();
        peer.conn = Some(Held {
            id: conn_id,
            by_me,
            tx,
        });
        peer.retry_at = None;
        let route = Route::of(established.peer.remote.ip());
        let remote = established.peer.remote;
        let reported: Vec<SocketAddr> = established
            .peer
            .handshake
            .addresses
            .iter()
            .filter_map(|a| a.parse().ok())
            .collect();
        self.shared.set_dialled(&id, Some(by_me));
        self.shared.update(&id, true, |l| {
            l.reachable = true;
            l.paused = false;
        });
        let remember = id.clone();
        let stored = blocking(&self.shared.host, move |conn| {
            peer_state::saw(conn, &remember, route)?;
            peer_state::learn(conn, &remember, &reported)?;
            if by_me {
                peer_state::worked(conn, &remember, remote)?;
            }
            Ok(())
        })
        .await;
        if let Err(e) = stored {
            tracing::warn!(target: "sync", error = %e, "could not remember the device");
        }
        let connection = Conn {
            shared: Arc::clone(&self.shared),
            peer: id.clone(),
            transport: established.transport,
            cmds,
            done: self.done_tx.clone(),
            keep_open: automatic,
        };
        let peer_id = id.clone();
        self.set.spawn(async move {
            let result = conn::run(connection).await;
            Done::Conn {
                peer: peer_id,
                id: conn_id,
                result,
            }
        });
        tracing::info!(target: "sync", by_me, "connected to a paired device");
    }

    async fn discovered(&mut self, candidate: Candidate) {
        if candidate.device == self.shared.me {
            return;
        }
        let automatic = self.automatic();
        let Some(peer) = self.peers.get_mut(&candidate.device) else {
            return;
        };
        let changed = !candidate.addrs.is_empty() && peer.found != candidate.addrs;
        if changed {
            peer.found.clone_from(&candidate.addrs);
        }
        // Seeing the device again is the best reason to try now, backoff or not; but a device
        // that keeps announcing the same addresses while it will not answer is left to the backoff.
        if automatic
            && peer.conn.is_none()
            && !peer.dialling
            && (changed || peer.retry_at.is_none())
        {
            peer.backoff.reset();
            self.dial(&candidate.device).await;
        }
    }

    async fn sync_now(&mut self) {
        let mut to_dial = Vec::new();
        for (id, peer) in &mut self.peers {
            peer.manual = true;
            match &peer.conn {
                Some(held) => {
                    let _ = held.tx.send(ConnCmd::Round);
                }
                None if !peer.dialling => {
                    peer.backoff.reset();
                    to_dial.push(id.clone());
                }
                None => {}
            }
        }
        for id in to_dial {
            self.dial(&id).await;
        }
    }

    async fn resume(&mut self) {
        for peer in self.peers.values_mut() {
            peer.backoff.reset();
        }
        if self.automatic() {
            self.dial_all().await;
        }
    }

    async fn reconfigure(&mut self) {
        if self.automatic() {
            for peer in self.peers.values_mut() {
                peer.backoff.reset();
            }
            self.dial_all().await;
            return;
        }
        // Off: nothing is kept open or retried; "Sync now" is the way to sync.
        let mut closed = Vec::new();
        for (id, peer) in &mut self.peers {
            peer.retry_at = None;
            if !peer.manual
                && let Some(held) = &peer.conn
            {
                let _ = held.tx.send(ConnCmd::Close);
                closed.push(id.clone());
            }
        }
        for id in closed {
            self.shared.update(&id, true, |l| {
                l.reachable = false;
                l.syncing = false;
                l.progress = None;
            });
        }
    }

    /// New local changes have been quiet for the debounce: every open connection starts a session.
    fn push(&self) {
        if !self.automatic() {
            return;
        }
        for peer in self.peers.values() {
            if let Some(held) = &peer.conn {
                let _ = held.tx.send(ConnCmd::Round);
            }
        }
    }
}
