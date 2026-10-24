//! The supervisor: one Tokio task that owns every dial and every connection of a running Sync,
//! decides who to dial and when, resolves two connections to the same device, and starts
//! sessions when this device has changes to push. Everything it spawns lives in its [`JoinSet`],
//! so aborting the supervisor (dropping [`Live`]) ends it all.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::runtime::Handle;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;
use tokio::task::{self, JoinError, JoinHandle, JoinSet};
use tokio::time::{Instant, sleep_until};

use super::backoff::Backoff;
use super::conn::{self, Conn, ConnCmd, RoundDone};
use super::dial::{candidates, race};
use super::shared::Shared;
use super::status::PeerLive;
use super::tie::replaces;
use super::{Host, Pace, blocking};
use rusqlite::Connection;

use crate::activity::{self, Local, Why};
use crate::discovery::{Browsing, Candidate, Discovery};
use crate::error::SyncError;
use crate::local_network::LocalNetwork;
use crate::net::{Established, NetConfig};
use crate::pause::Pause;
use crate::peer_state::{self, Route};
use crate::{peers, revoke};

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

/// What leaving the group needs of the supervisor.
#[derive(Clone)]
pub struct Leaver(UnboundedSender<Cmd>);

impl Leaver {
    /// Pushes what this device has written to every connected device, one session each, and waits
    /// for them (at most `limit` in all). Returns how many finished. With nobody connected, or
    /// nobody answering in time, it returns at once or at the limit with 0.
    pub async fn farewell(&self, limit: std::time::Duration) -> usize {
        let deadline = Instant::now() + limit;
        tell_everyone(deadline, |told| {
            let (tx, rx) = oneshot::channel();
            let sent = self.0.send(Cmd::Farewell(told, tx)).is_ok();
            async move {
                if !sent {
                    return None;
                }
                rx.await.ok()
            }
        })
        .await
    }
}

/// Asks (through `ask`, given the devices already told) for one farewell session per connection
/// of a device not yet told, and waits for them until `deadline`. A connection that ends before
/// its session ran (the other device dialled at the same moment and its connection replaced this
/// one) drops its answer unsent, which is not "told": the ask is made again, to whatever
/// connection stands now, until every answer arrives, nobody is connected any more, or the
/// deadline passes. Devices that did answer are not asked twice. Returns how many were told.
async fn tell_everyone<F, Fut>(deadline: Instant, mut ask: F) -> usize
where
    F: FnMut(HashSet<String>) -> Fut,
    Fut: std::future::Future<Output = Option<Vec<(String, oneshot::Receiver<()>)>>>,
{
    let mut told = HashSet::new();
    loop {
        let Ok(Some(pending)) = tokio::time::timeout_at(deadline, ask(told.clone())).await else {
            return told.len();
        };
        // The sessions run side by side, so waiting for them one after the other costs no more
        // than the slowest.
        let mut lost = false;
        for (peer, done) in pending {
            match tokio::time::timeout_at(deadline, done).await {
                Ok(Ok(())) => {
                    told.insert(peer);
                }
                Ok(Err(_)) => lost = true,
                Err(_) => return told.len(),
            }
        }
        if !lost {
            return told.len();
        }
        // Give the replacing connection a moment to be the one held.
        if tokio::time::timeout_at(
            deadline,
            tokio::time::sleep(std::time::Duration::from_millis(50)),
        )
        .await
        .is_err()
        {
            return told.len();
        }
    }
}

enum Cmd {
    /// A connection another device opened to us.
    Accept(Box<Established>),
    /// A connection we opened (a test hands one over; dials adopt their own).
    #[cfg(test)]
    Adopt(Box<Established>),
    SyncNow,
    /// "Sync now" for one device.
    SyncNowOne(String),
    Resume,
    /// Push now, without waiting out the debounce (the app is going to the background).
    Flush,
    Refresh,
    Reconfigure,
    /// This device is leaving: every open connection to a device not in the set runs one more
    /// session; the reply holds one receiver per connection, each ready when that session has
    /// finished.
    Farewell(
        HashSet<String>,
        oneshot::Sender<Vec<(String, oneshot::Receiver<()>)>>,
    ),
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
        local: Arc<LocalNetwork>,
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
            tasks: HashMap::new(),
            // Right away: on a Mac this is what raises the system's prompt, as Sync is turned on.
            probe_at: local.has_probe().then(Instant::now),
            local,
            push_at: None,
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

    /// "Sync now" for one device: syncs with it now if it can be reached, whatever "Sync
    /// automatically" says.
    pub fn sync_now_with(&self, device: &str) {
        let _ = self.cmds.send(Cmd::SyncNowOne(device.to_owned()));
    }

    /// The network changed or the app came back: try unreachable devices again from the start of
    /// the backoff.
    pub fn resume(&self) {
        let _ = self.cmds.send(Cmd::Resume);
    }

    /// Starts a session on every open connection now instead of after the debounce, if Sync
    /// automatically is on: the app is going to the background and pending changes should get out.
    pub fn flush(&self) {
        let _ = self.cmds.send(Cmd::Flush);
    }

    /// A cheap handle to say farewell with (see [`Leaver::farewell`]); it does not borrow `self`.
    pub fn leaver(&self) -> Leaver {
        Leaver(self.cmds.clone())
    }

    /// A session is running with some device right now.
    pub fn busy(&self) -> bool {
        self.shared.snapshot().values().any(|p| p.syncing)
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
        established: Option<Box<Established>>,
    },
    Conn {
        peer: String,
        id: u64,
        result: Result<(), SyncError>,
    },
}

/// What a task in the supervisor's set is doing, to put right what it was doing if it dies.
enum Doing {
    Dial(String),
    Conn(String, u64),
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
    tasks: HashMap<task::Id, Doing>,
    /// Whether the system keeps this device off the local network, and the probe that checks.
    local: Arc<LocalNetwork>,
    probe_at: Option<Instant>,
    /// When the next session round starts for the open connections (the quiet period after a
    /// change, local or just received).
    push_at: Option<Instant>,
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

/// Whether two lists name the same addresses. Discovery reports a device's addresses from a hash
/// set, so the same set can come in any order; only a real change may cut a backoff short.
fn same_addresses(a: &[SocketAddr], b: &[SocketAddr]) -> bool {
    a.len() == b.len() && a.iter().all(|x| b.contains(x))
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
        loop {
            self.shared
                .tasks
                .store(1 + self.set.len(), Ordering::SeqCst);
            let retry_at = self.peers.values().filter_map(|p| p.retry_at).min();
            tokio::select! {
                Some(cmd) = cmds.recv() => self.command(cmd).await,
                Some(round) = rounds.recv() => self.round_done(&round).await,
                Some(done) = self.set.join_next_with_id() => self.finished(done).await,
                () = changes.notified() => self.push_at = Some(Instant::now() + self.pace.debounce),
                found = next_found(&mut browse) => match found {
                    Some(candidate) => self.discovered(candidate).await,
                    None => browse = None,
                },
                () = wait_until(self.push_at) => {
                    self.push_at = None;
                    self.push();
                }
                () = wait_until(retry_at) => self.retry().await,
                () = wait_until(self.probe_at) => self.probe().await,
                () = self.local.changed() => self.local_network_changed().await,
            }
        }
    }

    /// Runs the local network probe and schedules the next one.
    async fn probe(&mut self) {
        let worked = self.local.run_probe().await;
        let wait = self.local.next_probe_in(worked);
        self.probe_at = Some(Instant::now() + wait);
    }

    /// Whether the system keeps this device off the local network just changed: the Devices tab
    /// says so, and Activity gets one line when it starts.
    async fn local_network_changed(&mut self) {
        self.shared.host.status_changed();
        if self.local.blocked() {
            self.note(activity::log_local_network_blocked).await;
        }
    }

    async fn command(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Accept(established) => self.adopt(*established, false).await,
            #[cfg(test)]
            Cmd::Adopt(established) => self.adopt(*established, true).await,
            Cmd::SyncNow => self.sync_now().await,
            Cmd::SyncNowOne(id) => self.sync_now_one(&id).await,
            Cmd::Resume => {
                if self.local.has_probe() {
                    self.probe_at = Some(Instant::now());
                }
                self.resume().await;
            }
            Cmd::Flush => {
                self.push_at = None;
                self.push();
            }
            Cmd::Refresh => {
                self.reload().await;
                if self.automatic() {
                    self.dial_all().await;
                }
            }
            Cmd::Reconfigure => self.reconfigure().await,
            Cmd::Farewell(told, reply) => {
                let mut waits = Vec::new();
                for (id, peer) in &self.peers {
                    if told.contains(id) {
                        continue;
                    }
                    if let Some(held) = &peer.conn {
                        let (tx, rx) = oneshot::channel();
                        if held.tx.send(ConnCmd::Farewell(tx)).is_ok() {
                            waits.push((id.clone(), rx));
                        }
                    }
                }
                let _ = reply.send(waits);
            }
        }
    }

    /// Reads the paired devices: new ones are tracked, forgotten ones dropped.
    async fn reload(&mut self) {
        let (ids, pinned, revoked) = blocking(self.host(), |conn| {
            // A device the group has forgotten syncs with nobody: it dials no one, answers no one.
            if revoke::removed_here(conn)? {
                return Ok((Vec::new(), Vec::new(), Vec::new()));
            }
            let ids = peers::list(conn)?
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<_>>();
            Ok((ids, peers::pinned(conn)?, revoke::revoked_keys(conn)?))
        })
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(target: "sync", error = %e, "could not read the paired devices");
            (Vec::new(), Vec::new(), Vec::new())
        });
        // What the paired devices and the forgotten ones say is what is trusted: a device taken
        // back is pinned again, one forgotten (here, or by another member) is refused from now on.
        for (key, device) in &pinned {
            self.config.trust.pin(*key, device);
        }
        for (key, _) in &revoked {
            self.config.trust.unpin(key);
        }
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
        let typed = blocking(self.host(), {
            let device = device.clone();
            move |conn| crate::detail::typed_for_dial(conn, &device)
        })
        .await
        .unwrap_or_default();
        let (config, each) = (self.config.clone(), self.pace.dial_each);
        let local = Arc::clone(&self.local);
        let handle = self.set.spawn(async move {
            // The Address a person typed is the third source, looked up here so a slow name never
            // holds the others up.
            let typed = match typed {
                Some(address) => address.resolve(each).await.unwrap_or_default(),
                None => Vec::new(),
            };
            let addrs = candidates(&found, &remembered, &typed);
            let established = if addrs.is_empty() {
                None
            } else {
                race(&config, addrs, each, &local).await.map(Box::new)
            };
            Done::Dial {
                peer: device,
                established,
            }
        });
        self.tasks.insert(handle.id(), Doing::Dial(id.to_owned()));
    }

    fn dial_failed(&mut self, id: &str) {
        let auto = self.automatic();
        let Some(peer) = self.peers.get_mut(id) else {
            return;
        };
        peer.dialling = false;
        if peer.conn.is_some() {
            // The other device's own connection was adopted while this dial raced (both dialled
            // at once): the device is reachable, and a failed dial says nothing against it.
            return;
        }
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

    async fn finished(&mut self, done: Result<(task::Id, Done), JoinError>) {
        let done = match done {
            Ok((id, done)) => {
                self.tasks.remove(&id);
                done
            }
            Err(e) => return self.task_died(&e).await,
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
                        self.adopt(*established, true).await;
                    }
                    None => self.dial_failed(&peer),
                }
            }
            Done::Conn { peer, id, result } => self.connection_ended(&peer, id, result).await,
        }
    }

    /// A dial or connection task panicked (or was cancelled): its device must not stay marked as
    /// dialling or connected for good.
    async fn task_died(&mut self, error: &JoinError) {
        tracing::warn!(target: "sync", panicked = error.is_panic(), "a sync task ended unexpectedly");
        match self.tasks.remove(&error.id()) {
            Some(Doing::Dial(peer)) => self.dial_failed(&peer),
            Some(Doing::Conn(peer, id)) => {
                self.connection_ended(&peer, id, Err(SyncError::Disconnected))
                    .await;
            }
            None => {}
        }
    }

    async fn connection_ended(&mut self, id_of_peer: &str, id: u64, result: Result<(), SyncError>) {
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
                if let SyncError::ClockAhead { resumes_in_ms } = e {
                    // A record from further ahead than five minutes was refused: the peer's
                    // clock (or that of a device it relays) was wrong, whatever its handshake
                    // says now.
                    self.shared
                        .update(id_of_peer, true, |l| l.reachable = false);
                    self.pause(id_of_peer, Pause::Stamped { resumes_in_ms })
                        .await;
                    return;
                }
                // Only a line lost in the middle of a sync is news.
                let mid_sync = self
                    .shared
                    .snapshot()
                    .get(id_of_peer)
                    .is_some_and(|l| l.syncing);
                self.shared.update(id_of_peer, true, |l| {
                    l.reachable = false;
                    l.syncing = false;
                    l.progress = None;
                });
                if mid_sync {
                    let peer = id_of_peer.to_owned();
                    self.note(move |conn, local| {
                        activity::log_failed(conn, local, &peer, Why::Lost)
                    })
                    .await;
                }
            }
        }
    }

    /// Whether `id` is paused for a reason a good handshake does not end (see `Pause::Stamped`).
    fn outlasts_handshake(&self, id: &str) -> bool {
        self.shared
            .snapshot()
            .get(id)
            .and_then(|l| l.paused.as_ref())
            .is_some_and(Pause::outlives_handshake)
    }

    /// Marks `id` paused for `pause`: the Devices tab shows it, Activity says so once.
    async fn pause(&self, id: &str, pause: Pause) {
        self.shared.update(id, true, |l| {
            l.paused = Some(pause.clone());
            l.syncing = false;
            l.progress = None;
        });
        self.note_pause(id, Some(&pause)).await;
    }

    /// A device is met paused or running again: one line in Activity per change, which the log
    /// itself keeps track of (so a restart does not log it twice).
    async fn note_pause(&self, peer: &str, paused: Option<&Pause>) {
        let peer = peer.to_owned();
        if let Some(pause) = paused {
            let why = match pause {
                Pause::Update { .. } => Why::Update,
                Pause::Clock { .. } | Pause::Stamped { .. } => Why::Clock,
            };
            self.note(move |conn, local| activity::log_paused(conn, local, &peer, why))
                .await;
        } else {
            self.note(move |conn, local| activity::log_resumed(conn, local, &peer))
                .await;
        }
    }

    /// Writes a line in Activity (the supervisor's own events: a pause, a resume, a lost line).
    /// A failure to write costs the line only.
    async fn note(
        &self,
        f: impl FnOnce(&Connection, Local<'_>) -> Result<bool, SyncError> + Send + 'static,
    ) {
        let me = self.shared.me.clone();
        let name = self.shared.host.device_name();
        let made = blocking(&self.shared.host, move |conn| {
            f(
                conn,
                Local {
                    device: &me,
                    name: &name,
                },
            )
        })
        .await;
        match made {
            Ok(true) => self.shared.host.activity_changed(),
            Ok(false) => {}
            Err(e) => tracing::warn!(target: "sync", error = %e, "could not log to Activity"),
        }
    }

    async fn round_done(&mut self, round: &RoundDone) {
        if let Some(peer) = self.peers.get_mut(&round.peer) {
            peer.backoff.reset();
            peer.manual = false;
        }
        // A sync that ran to the end: whatever was wrong with the records is gone.
        let stamped = self.outlasts_handshake(&round.peer);
        if stamped {
            self.shared.update(&round.peer, true, |l| l.paused = None);
            self.note_pause(&round.peer, None).await;
        }
        if round.received > 0 {
            // What arrived may have forgotten a device (or taken one back): the connection to it
            // closes now, not at the next round.
            self.reload().await;
            // What arrived may be news to the other devices too (one of them may not reach the
            // sender). Nothing hooks the sync connection's own writes, so say so here. A round
            // that received nothing ends the chain.
            self.push_at = Some(Instant::now() + self.pace.debounce);
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
        if let Some(pause) = established.peer.pause.clone() {
            // Different data shapes or clocks too far apart: no session either way, and the
            // usual backoff so neither side hammers the other. The Devices tab says why.
            tracing::info!(target: "sync", modules = ?established.peer.data_version_mismatch, clock = matches!(pause, Pause::Clock { .. }), "paused");
            if automatic && by_me {
                peer.retry_at = Some(Instant::now() + peer.backoff.next_delay());
            }
            self.pause(&id, pause).await;
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
        // A pause caused by what was stored on the other side outlives a good handshake: the
        // retry would otherwise log a resume and a pause every time. A finished sync ends it.
        let kept = self.outlasts_handshake(&id);
        self.shared.update(&id, true, |l| {
            l.reachable = true;
            if !kept {
                l.paused = None;
            }
        });
        if !kept {
            self.note_pause(&id, None).await;
        }
        let remember = id.clone();
        let (app_version, versions) = (
            established.peer.handshake.app_version.clone(),
            established.peer.handshake.data_versions.clone(),
        );
        let stored = blocking(&self.shared.host, move |conn| {
            peer_state::saw(conn, &remember, route)?;
            peer_state::met(conn, &remember, &app_version, &versions)?;
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
            backed_up: false,
        };
        let peer_id = id.clone();
        let handle = self.set.spawn(async move {
            let result = conn::run(connection).await;
            Done::Conn {
                peer: peer_id,
                id: conn_id,
                result,
            }
        });
        self.tasks.insert(handle.id(), Doing::Conn(id, conn_id));
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
        let changed = !candidate.addrs.is_empty() && !same_addresses(&peer.found, &candidate.addrs);
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

    async fn sync_now_one(&mut self, id: &str) {
        let Some(peer) = self.peers.get_mut(id) else {
            return;
        };
        peer.manual = true;
        match &peer.conn {
            Some(held) => {
                let _ = held.tx.send(ConnCmd::Round);
            }
            None if !peer.dialling => {
                peer.backoff.reset();
                self.dial(id).await;
            }
            None => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn the_same_addresses_in_another_order_are_not_a_change() {
        let one = [a("192.168.1.9:1"), a("100.64.0.2:1")];
        let other = [a("100.64.0.2:1"), a("192.168.1.9:1")];
        assert!(same_addresses(&one, &other));
        assert!(!same_addresses(&one, &other[..1]));
        assert!(!same_addresses(
            &one,
            &[a("192.168.1.9:1"), a("10.0.0.5:1")]
        ));
        assert!(same_addresses(&[], &[]));
    }

    use std::time::Duration;

    fn answered() -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        tx.send(()).unwrap();
        rx
    }

    fn peer(id: &str, rx: oneshot::Receiver<()>) -> (String, oneshot::Receiver<()>) {
        (id.to_string(), rx)
    }

    #[tokio::test]
    async fn a_connection_replaced_before_its_farewell_ran_is_asked_again_but_a_told_one_is_not() {
        let mut asked = Vec::new();
        let finished = tell_everyone(Instant::now() + Duration::from_secs(5), |told| {
            let mut told: Vec<_> = told.into_iter().collect();
            told.sort();
            asked.push(told.clone());
            let first = asked.len() == 1;
            async move {
                // "b" was closed unanswered (its sender dropped) and its replacement answers;
                // "a" answered at once and must not be asked again.
                Some(if first {
                    vec![peer("a", answered()), peer("b", oneshot::channel().1)]
                } else {
                    assert_eq!(told, ["a"]);
                    vec![peer("b", answered())]
                })
            }
        })
        .await;
        assert_eq!((asked.len(), finished), (2, 2));
    }

    #[tokio::test]
    async fn nobody_connected_any_more_ends_the_farewell() {
        let mut asked = 0;
        let finished = tell_everyone(Instant::now() + Duration::from_secs(5), |_| {
            asked += 1;
            let first = asked == 1;
            async move {
                Some(if first {
                    vec![peer("a", oneshot::channel().1)]
                } else {
                    Vec::new()
                })
            }
        })
        .await;
        assert_eq!((asked, finished), (2, 0));
    }

    #[tokio::test]
    async fn a_session_that_never_answers_stops_at_the_deadline() {
        let (_keep, silent) = oneshot::channel::<()>();
        let mut silent = Some(silent);
        let started = std::time::Instant::now();
        let finished = tell_everyone(Instant::now() + Duration::from_millis(150), |_| {
            let pending = silent.take().map(|rx| peer("a", rx)).into_iter().collect();
            async move { Some(pending) }
        })
        .await;
        assert_eq!(finished, 0);
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
