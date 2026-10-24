//! [`Pairing`]: the state of the pairing sheet and the tasks behind it. The UI calls `show`,
//! `enter`, `decide`, `new_code` and `cancel`; every change is published to the host as a
//! [`PairingView`]. While a code is shown, TLS admits unpinned keys and the listener hands them
//! here; while one is typed, the same switch lets this device dial a device that is not pinned.

use std::collections::HashSet;
use std::future::Future;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use mnemora_core::syncable::Holdings;
use tokio::runtime::Handle;
use tokio::sync::oneshot;
use tokio::task::AbortHandle;
use tokio::time::{sleep, timeout, timeout_at};

use super::Timing;
use super::code::{self, MAX_TRIES};
use super::host::{Paired, PairingHost, Stored};
use super::msg::{Reason, Who};
use super::preview::{Counts, lists_that_combine};
use super::proto::{self, Attempt, Choice, Decision, Failure, ShowerPort, TyperPort};
use super::view::{PairingError, PairingView};
use crate::SyncError;
use crate::address::{Address, ResolveError};
use crate::discovery::{Advert, Discovery};
use crate::net::{NetConfig, Secured, secure_dial};

/// A code on display.
struct Show {
    code: String,
    expires: Instant,
    tries_left: u8,
    /// Why the code no longer works, once it does not.
    dead: Option<PairingError>,
    /// The last attempt that failed for a reason other than a wrong code.
    notice: Option<PairingError>,
}

/// The question to the person on the showing device.
struct Pending {
    id: u64,
    who: Who,
    answer: Option<oneshot::Sender<Decision>>,
    /// Allow was pressed; the pairing is being recorded.
    answered: bool,
    /// The typing device's person is deciding on the merge preview.
    confirming: bool,
    /// Dropped (so the receiver resolves) when the pairing is stopped while confirming.
    stop: Option<oneshot::Sender<()>>,
}

struct Inner {
    /// What the sheet shows when no code is on display and nobody is asking.
    base: PairingView,
    show: Option<Show>,
    asking: Option<Pending>,
    typing: Option<AbortHandle>,
    /// The merge preview is on the sheet (typing side): where the person's choice goes.
    reviewing: Option<oneshot::Sender<Choice>>,
    timer: Option<AbortHandle>,
    /// Bumps whenever a code is made or pairing is cancelled, so a late task knows it is stale.
    generation: u64,
    next_question: u64,
}

impl Inner {
    fn show_alive(&self) -> bool {
        self.show.as_ref().is_some_and(|s| s.dead.is_none())
    }

    /// Whether a typing device is answered: a live code, or one that ran out (and says so).
    fn show_answers(&self) -> bool {
        self.show.as_ref().is_some_and(|s| {
            s.dead
                .as_ref()
                .is_none_or(|e| matches!(e, PairingError::CodeExpired { .. }))
        })
    }
}

/// Pairing for one running Sync: see the module docs.
pub struct Pairing {
    host: Arc<dyn PairingHost>,
    discovery: Arc<dyn Discovery>,
    config: NetConfig,
    port: u16,
    timing: Timing,
    runtime: Handle,
    tasks: Arc<AtomicUsize>,
    inner: Mutex<Inner>,
}

struct TaskCount(Arc<AtomicUsize>);

impl Drop for TaskCount {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn millis(d: Duration) -> u32 {
    u32::try_from(d.as_millis()).unwrap_or(u32::MAX)
}

impl Pairing {
    /// Pairing over `config`'s identity and trust set, listening on `port`. Tasks run on `runtime`.
    pub fn new(
        host: Arc<dyn PairingHost>,
        discovery: Arc<dyn Discovery>,
        config: NetConfig,
        port: u16,
        timing: Timing,
        runtime: Handle,
    ) -> Arc<Self> {
        Arc::new(Self {
            host,
            discovery,
            config,
            port,
            timing,
            runtime,
            tasks: Arc::new(AtomicUsize::new(0)),
            inner: Mutex::new(Inner {
                base: PairingView::Idle,
                show: None,
                asking: None,
                typing: None,
                reviewing: None,
                timer: None,
                generation: 0,
                next_question: 0,
            }),
        })
    }

    /// Live pairing tasks (timer, typing, connections being served).
    pub fn tasks(&self) -> usize {
        self.tasks.load(Ordering::SeqCst)
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) -> AbortHandle {
        self.tasks.fetch_add(1, Ordering::SeqCst);
        let count = TaskCount(Arc::clone(&self.tasks));
        self.runtime
            .spawn(async move {
                let _count = count;
                task.await;
            })
            .abort_handle()
    }

    /// This device as it introduces itself, with its key.
    fn me(&self) -> Result<Who, SyncError> {
        let mut me = self.host.me()?;
        me.key = self.config.identity.public_key().to_hex();
        Ok(me)
    }

    // ---- what the sheet shows

    fn view_of(&self, g: &Inner) -> PairingView {
        if let Some(p) = &g.asking {
            return if p.confirming {
                PairingView::Confirming {
                    name: p.who.name.clone(),
                }
            } else if p.answered {
                PairingView::Finishing {
                    name: p.who.name.clone(),
                }
            } else {
                PairingView::Asking {
                    name: p.who.name.clone(),
                    platform: p.who.platform,
                    life_ms: millis(self.timing.code_life),
                    left_ms: g.show.as_ref().map_or(0, |s| {
                        millis(s.expires.saturating_duration_since(Instant::now()))
                    }),
                }
            };
        }
        match &g.show {
            Some(s) => match &s.dead {
                Some(error) => PairingView::Failed {
                    error: error.clone(),
                },
                None => PairingView::Showing {
                    code: code::display(&s.code),
                    life_ms: millis(self.timing.code_life),
                    left_ms: millis(s.expires.saturating_duration_since(Instant::now())),
                    tries_left: s.tries_left,
                    discoverable: self.host.visible(),
                    notice: s.notice.clone(),
                },
            },
            None => g.base.clone(),
        }
    }

    /// Whether TLS currently admits unpinned keys (a code is shown or being typed).
    #[cfg(test)]
    pub(crate) fn state_is_open(&self) -> bool {
        self.config.trust.pairing_open()
    }

    /// The state right now.
    pub fn state(&self) -> PairingView {
        let g = self.lock();
        self.view_of(&g)
    }

    /// Tells the host. Called with the lock held so that events keep the order of the changes.
    fn publish(&self, g: &Inner) {
        self.host.show(&self.view_of(g));
    }

    /// TLS admits unpinned keys while a code is shown or being typed, and while a code that ran
    /// out is still on the sheet: such a connection is only told the code is dead, so the typing
    /// device can say so instead of "nothing answered".
    fn sync_open(&self, g: &Inner) {
        self.config
            .trust
            .set_pairing_open(g.show_answers() || g.typing.is_some());
    }

    /// Announces this device again (the "pairing open" flag, the group, Visible on this network).
    pub fn refresh_advert(&self) {
        let open = self.lock().show_alive();
        let advert = if self.host.visible() {
            self.host.me().ok().map(|me| Advert {
                device: me.device,
                group: me.group,
                port: self.port,
                pairing_open: open,
            })
        } else {
            None
        };
        self.discovery.advertise(advert.as_ref());
    }

    // ---- showing a code

    /// Stops whatever is going on: no code, no typing, nobody asked.
    fn clear(g: &mut Inner, tell: Decision) {
        g.generation += 1;
        if let Some(t) = g.typing.take() {
            t.abort();
        }
        if let Some(t) = g.timer.take() {
            t.abort();
        }
        g.reviewing = None;
        if let Some(answer) = g.asking.take().and_then(|mut p| p.answer.take()) {
            let _ = answer.send(tell);
        }
        g.show = None;
        g.base = PairingView::Idle;
    }

    /// Shows a new code and opens pairing.
    pub fn show(self: &Arc<Self>) -> Result<(), SyncError> {
        let code =
            code::generate().ok_or(SyncError::Validation("This device cannot make a code"))?;
        {
            let mut g = self.lock();
            Self::clear(&mut g, Decision::Cancelled);
            self.start_code(&mut g, code);
        }
        self.refresh_advert();
        Ok(())
    }

    fn start_code(self: &Arc<Self>, g: &mut Inner, code: String) {
        let generation = g.generation;
        g.show = Some(Show {
            code,
            expires: Instant::now() + self.timing.code_life,
            tries_left: MAX_TRIES,
            dead: None,
            notice: None,
        });
        let me = Arc::clone(self);
        let life = self.timing.code_life;
        g.timer = Some(self.spawn(async move {
            sleep(life).await;
            me.expire(generation);
        }));
        self.sync_open(g);
        self.publish(g);
    }

    /// "New code": a fresh code, 3 tries and 2 minutes, unless someone is being asked right now.
    pub fn new_code(self: &Arc<Self>) -> Result<(), SyncError> {
        let code =
            code::generate().ok_or(SyncError::Validation("This device cannot make a code"))?;
        {
            let mut g = self.lock();
            if g.show.is_none() || g.asking.is_some() {
                return Ok(());
            }
            Self::clear(&mut g, Decision::Cancelled);
            self.start_code(&mut g, code);
        }
        self.refresh_advert();
        Ok(())
    }

    fn expire(&self, generation: u64) {
        {
            let mut g = self.lock();
            // Once Allow was pressed the pairing runs to its end (the merge preview may take
            // longer than the code); `end_question` closes a code that ran out meanwhile.
            if g.generation != generation
                || !g.show_alive()
                || g.asking.as_ref().is_some_and(|p| p.answered)
            {
                return;
            }
            if let Some(show) = g.show.as_mut() {
                show.dead = Some(PairingError::CodeExpired { name: None });
            }
            if let Some(answer) = g.asking.take().and_then(|mut p| p.answer.take()) {
                let _ = answer.send(Decision::Expired);
            }
            self.sync_open(&g);
            self.publish(&g);
        }
        self.refresh_advert();
    }

    /// Allow or Deny, from the question on the showing device.
    pub fn decide(&self, allow: bool) {
        let mut g = self.lock();
        let Some(p) = g.asking.as_mut() else { return };
        let Some(answer) = p.answer.take() else {
            return;
        };
        p.answered = allow;
        let _ = answer.send(if allow {
            Decision::Allow
        } else {
            Decision::Deny
        });
        self.publish(&g);
    }

    /// The typing device's person chose on the merge preview. Cancel tells the other device and
    /// leaves both databases as they were.
    pub fn confirm(&self, merge: bool) {
        let mut g = self.lock();
        if let Some(choice) = g.reviewing.take() {
            let _ = choice.send(if merge { Choice::Merge } else { Choice::Cancel });
        }
    }

    /// Closes the sheet's pairing: no code, no search, and whoever was asked is told.
    pub fn cancel(&self) {
        {
            let mut g = self.lock();
            // On the merge preview the exchange itself says no to the other device, then the
            // sheet closes.
            if let Some(choice) = g.reviewing.take() {
                let _ = choice.send(Choice::Cancel);
                return;
            }
            Self::clear(&mut g, Decision::Cancelled);
            self.sync_open(&g);
            self.publish(&g);
        }
        self.refresh_advert();
    }

    /// Stops everything for good (Sync turned off). Publishes nothing.
    pub fn shutdown(&self) {
        {
            let mut g = self.lock();
            Self::clear(&mut g, Decision::Cancelled);
            self.config.trust.set_pairing_open(false);
        }
        self.discovery.advertise(None);
    }

    /// The listener handed over a connection from an unpinned key.
    pub(crate) fn handle_incoming(self: &Arc<Self>, secured: Box<Secured>) {
        if !self.lock().show_answers() {
            tracing::debug!(target: "sync", "no code is shown, dropping a pairing attempt");
            return;
        }
        let me = Arc::clone(self);
        self.spawn(async move { me.serve(*secured).await });
    }

    async fn serve(self: Arc<Self>, secured: Secured) {
        let Secured {
            mut channel, key, ..
        } = secured;
        let Ok(me) = self.me() else { return };
        let side = ShowSide::new(&self);
        let result = proto::show_code(&mut channel, &side, &me, key, &self.timing).await;
        self.end_question(side.question());
        match result {
            Ok(stored) => self.finish_shower(&stored),
            Err(Failure::Refused {
                reason: Reason::Declined,
                ..
            }) => self.cancel(),
            Err(Failure::Environment { theirs }) => {
                self.notice(PairingError::DifferentEnvironment { theirs });
            }
            Err(Failure::Version {
                name,
                older,
                older_here,
                older_there,
            }) => {
                self.notice(PairingError::VersionMismatch {
                    name,
                    older,
                    older_here,
                    older_there,
                });
            }
            Err(Failure::Refused {
                reason: Reason::Groups,
                ..
            }) => self.notice(PairingError::DifferentGroups),
            Err(Failure::Refused {
                reason: Reason::BackupFailed,
                name,
            }) => self.notice(PairingError::BackupFailed { name }),
            // The typing device's person said no to the merge: the code stays, with a note.
            Err(Failure::Refused {
                reason: Reason::Cancelled,
                name: Some(name),
            }) => self.notice(PairingError::Cancelled { name: Some(name) }),
            // A wrong code was counted when it happened; expiry, cancel and a lost connection
            // were published by whoever caused them.
            Err(_) => {}
        }
    }

    /// The question to the person is over (answered, refused or the connection went): takes it
    /// off the sheet, and closes the code if it ran out while the pairing was being finished.
    fn end_question(&self, id: u64) {
        if id == 0 {
            return;
        }
        let ran_out = {
            let mut g = self.lock();
            if g.asking.as_ref().is_none_or(|p| p.id != id) {
                return;
            }
            g.asking = None;
            let ran_out = g.show.as_mut().is_some_and(|s| {
                let out = s.dead.is_none() && s.expires <= Instant::now();
                if out {
                    s.dead = Some(PairingError::CodeExpired { name: None });
                }
                out
            });
            if ran_out && let Some(timer) = g.timer.take() {
                timer.abort();
            }
            self.sync_open(&g);
            self.publish(&g);
            ran_out
        };
        if ran_out {
            self.refresh_advert();
        }
    }

    fn notice(&self, error: PairingError) {
        let mut g = self.lock();
        if let Some(show) = g.show.as_mut().filter(|s| s.dead.is_none()) {
            show.notice = Some(error);
            self.publish(&g);
        }
    }

    fn finish_shower(&self, stored: &Stored) {
        {
            let mut g = self.lock();
            Self::clear(&mut g, Decision::Cancelled);
            g.base = paired_view(stored);
            self.sync_open(&g);
            self.publish(&g);
        }
        self.refresh_advert();
    }

    /// Records a pairing and pins the keys it brought.
    fn commit(&self, paired: &Paired, typed: Option<&str>) -> Result<Stored, SyncError> {
        let stored = self.host.store(paired)?;
        if let Some(address) = typed {
            self.host.remember_address(&paired.who.device, address);
        }
        for (key, device) in &stored.pins {
            self.config.trust.pin(*key, device);
        }
        tracing::info!(target: "sync", also_joined = stored.also_joined.len(), "paired a device");
        self.host.paired();
        Ok(stored)
    }

    // ---- typing a code

    /// Types a code: looks for devices showing one and tries it on each.
    pub fn enter(self: &Arc<Self>, input: &str) -> Result<(), SyncError> {
        let code =
            code::normalise(input).ok_or(SyncError::Validation("Type all 8 digits of the code"))?;
        let mut g = self.lock();
        Self::clear(&mut g, Decision::Cancelled);
        let generation = g.generation;
        g.base = PairingView::Searching;
        let me = Arc::clone(self);
        g.typing = Some(self.spawn(async move { me.type_and_finish(code, generation).await }));
        self.sync_open(&g);
        self.publish(&g);
        Ok(())
    }

    /// Pair by address: dials the typed `address` with the code. The other device need not be
    /// visible to discovery; on success the address becomes its typed Address.
    pub fn enter_at(self: &Arc<Self>, address: &str, input: &str) -> Result<(), SyncError> {
        let address = Address::parse(address).map_err(|e| SyncError::Validation(e.message()))?;
        let code =
            code::normalise(input).ok_or(SyncError::Validation("Type all 8 digits of the code"))?;
        let mut g = self.lock();
        Self::clear(&mut g, Decision::Cancelled);
        let generation = g.generation;
        g.base = PairingView::Searching;
        let me = Arc::clone(self);
        g.typing = Some(self.spawn(async move {
            let outcome = me.pair_at(&address, &code).await;
            me.finish_typing(outcome, generation);
            me.refresh_advert();
        }));
        self.sync_open(&g);
        self.publish(&g);
        Ok(())
    }

    async fn pair_at(&self, address: &Address, code: &str) -> Result<Stored, PairingError> {
        let me = self.me().map_err(|_| PairingError::NotReady)?;
        let addrs = address
            .resolve(self.timing.dial)
            .await
            .map_err(|e| match e {
                ResolveError::NotFound => PairingError::AddressNotFound,
                ResolveError::TimedOut => PairingError::AddressUnreachable,
            })?;
        let typed = address.normalized();
        for addr in addrs {
            match self.attempt(addr, &me, code, Some(&typed)).await {
                Err(PairingError::ConnectionLost) => {}
                other => return other,
            }
        }
        Err(PairingError::AddressUnreachable)
    }

    async fn type_and_finish(self: Arc<Self>, code: String, generation: u64) {
        let outcome = self.search_and_pair(&code).await;
        self.finish_typing(outcome, generation);
        self.refresh_advert();
    }

    fn finish_typing(&self, outcome: Result<Stored, PairingError>, generation: u64) {
        {
            let mut g = self.lock();
            if g.generation != generation {
                return;
            }
            g.typing = None;
            g.reviewing = None;
            g.base = match outcome {
                Ok(stored) => paired_view(&stored),
                // This device's own person cancelled the merge preview: the sheet just closes.
                Err(PairingError::Cancelled { name: None }) => PairingView::Idle,
                Err(error) => PairingView::Failed { error },
            };
            self.sync_open(&g);
            self.publish(&g);
        }
    }

    async fn search_and_pair(&self, code: &str) -> Result<Stored, PairingError> {
        let me = self.me().map_err(|_| PairingError::NotReady)?;
        let mut browsing = self
            .discovery
            .browse()
            .map_err(|_| PairingError::NothingNearby)?;
        let deadline = tokio::time::Instant::now() + self.timing.search;
        let mut tried = HashSet::new();
        let mut best: Option<PairingError> = None;
        while let Ok(Some(found)) = timeout_at(deadline, browsing.found.recv()).await {
            if !found.pairing_open
                || found.device == me.device
                || !tried.insert(found.device.clone())
            {
                continue;
            }
            let mut result = Err(PairingError::ConnectionLost);
            for addr in &found.addrs {
                result = self.attempt(*addr, &me, code, None).await;
                if !matches!(result, Err(PairingError::ConnectionLost)) {
                    break;
                }
            }
            match result {
                Ok(stored) => return Ok(stored),
                Err(error) if is_final(&error) => return Err(error),
                Err(error) => {
                    if best.as_ref().is_none_or(|b| rank(&error) > rank(b)) {
                        best = Some(error);
                    }
                }
            }
        }
        Err(best.unwrap_or(PairingError::NothingNearby))
    }

    async fn attempt(
        &self,
        addr: std::net::SocketAddr,
        me: &Who,
        code: &str,
        typed: Option<&str>,
    ) -> Result<Stored, PairingError> {
        let secured = match timeout(self.timing.dial, secure_dial(&self.config, addr)).await {
            Ok(Ok(secured)) => secured,
            Ok(Err(e)) => {
                tracing::debug!(target: "sync", error = %e, "could not reach a device to pair with");
                return Err(PairingError::ConnectionLost);
            }
            Err(_) => {
                tracing::debug!(target: "sync", "dialling a device to pair with timed out");
                return Err(PairingError::ConnectionLost);
            }
        };
        let Secured {
            mut channel, key, ..
        } = secured;
        proto::type_code(
            &mut channel,
            &TypeSide { hub: self, typed },
            me,
            key,
            code,
            &self.timing,
        )
        .await
        .map_err(typer_error)
    }
}

fn paired_view(stored: &Stored) -> PairingView {
    PairingView::Paired {
        name: stored.name.clone(),
        platform: stored.platform,
        renamed: stored.renamed,
        also_joined: stored.also_joined.clone(),
    }
}

/// An error that ends the search at once: the right code was typed, and something else is wrong.
fn is_final(error: &PairingError) -> bool {
    matches!(
        error,
        PairingError::Declined { .. }
            | PairingError::VersionMismatch { .. }
            | PairingError::DifferentGroups
            | PairingError::Cancelled { .. }
            | PairingError::BackupFailed { .. }
    )
}

/// Which error is most worth showing when several devices were tried.
fn rank(error: &PairingError) -> u8 {
    match error {
        PairingError::WrongCode { .. } => 5,
        PairingError::CodeExpired { .. } => 4,
        PairingError::DifferentEnvironment { .. } => 3,
        PairingError::Busy => 2,
        PairingError::ConnectionLost => 1,
        _ => 0,
    }
}

fn typer_error(failure: Failure) -> PairingError {
    match failure {
        Failure::WrongCode { tries_left, name } => PairingError::WrongCode { tries_left, name },
        Failure::Version {
            name,
            older,
            older_here,
            older_there,
        } => PairingError::VersionMismatch {
            name,
            older,
            older_here,
            older_there,
        },
        Failure::Environment { theirs } => PairingError::DifferentEnvironment { theirs },
        Failure::Lost => PairingError::ConnectionLost,
        Failure::Refused { reason, name } => match reason {
            Reason::Declined => PairingError::Declined {
                name: name.unwrap_or_default(),
            },
            Reason::Expired => PairingError::CodeExpired { name },
            Reason::Cancelled => PairingError::Cancelled { name },
            Reason::Busy => PairingError::Busy,
            Reason::BackupFailed => PairingError::BackupFailed { name },
            Reason::Groups => PairingError::DifferentGroups,
            Reason::Version => PairingError::VersionMismatch {
                name: name.unwrap_or_default(),
                older: super::view::Older::Them,
                older_here: Vec::new(),
                older_there: Vec::new(),
            },
            Reason::Environment { sender } => PairingError::DifferentEnvironment { theirs: sender },
        },
    }
}

/// The typing side's hooks into [`Pairing`].
struct TypeSide<'a> {
    hub: &'a Pairing,
    /// The address this pairing dialled, when the person typed it.
    typed: Option<&'a str>,
}

impl TyperPort for TypeSide<'_> {
    fn waiting(&self, shower: &Who) {
        let mut g = self.hub.lock();
        g.base = PairingView::Waiting {
            name: shower.name.clone(),
        };
        self.hub.publish(&g);
    }

    fn holdings(&self) -> Result<Holdings, SyncError> {
        self.hub.host.holdings()
    }

    fn review(
        &self,
        shower: &Who,
        mine: &Holdings,
        theirs: &Holdings,
    ) -> impl Future<Output = Choice> + Send {
        let (tx, rx) = oneshot::channel();
        {
            let mut g = self.hub.lock();
            g.reviewing = Some(tx);
            g.base = PairingView::Preview {
                name: shower.name.clone(),
                platform: shower.platform,
                there: Counts::of(theirs),
                here: Counts::of(mine),
                combine: lists_that_combine(mine, theirs),
            };
            self.hub.publish(&g);
        }
        async move { rx.await.unwrap_or(Choice::Cancel) }
    }

    fn backup(&self, shower: &Who) -> Result<(), SyncError> {
        {
            let mut g = self.hub.lock();
            g.base = PairingView::Finishing {
                name: shower.name.clone(),
            };
            self.hub.publish(&g);
        }
        self.hub.host.backup()
    }

    fn commit(&self, paired: &Paired) -> Result<Stored, SyncError> {
        self.hub.commit(paired, self.typed)
    }
}

/// The showing side's hooks into [`Pairing`].
pub(super) struct ShowSide<'a> {
    hub: &'a Arc<Pairing>,
    /// The question this connection asked (0: none yet), for `Pairing::end_question`.
    question: AtomicU64,
}

impl<'a> ShowSide<'a> {
    pub(super) fn new(hub: &'a Arc<Pairing>) -> Self {
        Self {
            hub,
            question: AtomicU64::new(0),
        }
    }

    fn question(&self) -> u64 {
        self.question.load(Ordering::SeqCst)
    }
}

impl ShowerPort for ShowSide<'_> {
    fn code(&self) -> Option<String> {
        let g = self.hub.lock();
        g.show
            .as_ref()
            .filter(|s| s.dead.is_none())
            .map(|s| s.code.clone())
    }

    fn attempt(&self, code: &str, proves: &dyn Fn() -> bool) -> Attempt {
        let left = {
            let mut g = self.hub.lock();
            let Some(show) = g
                .show
                .as_mut()
                .filter(|s| s.dead.is_none() && s.code == code)
            else {
                return Attempt::Dead;
            };
            if proves() {
                return Attempt::Right;
            }
            show.tries_left = show.tries_left.saturating_sub(1);
            show.notice = None;
            let left = show.tries_left;
            if left == 0 {
                show.dead = Some(PairingError::TriesUsed);
                if let Some(timer) = g.timer.take() {
                    timer.abort();
                }
            }
            self.hub.sync_open(&g);
            self.hub.publish(&g);
            left
        };
        if left == 0 {
            self.hub.refresh_advert();
        }
        Attempt::Wrong(left)
    }

    fn ask(&self, who: &Who) -> impl Future<Output = Decision> + Send {
        let hub = Arc::clone(self.hub);
        let who = who.clone();
        let question = &self.question;
        async move {
            let (tx, rx) = oneshot::channel();
            let id = {
                let mut g = hub.lock();
                if !g.show_alive() {
                    return Decision::Expired;
                }
                if g.asking.is_some() {
                    return Decision::Busy;
                }
                g.next_question += 1;
                let id = g.next_question;
                if let Some(show) = g.show.as_mut() {
                    show.notice = None;
                }
                g.asking = Some(Pending {
                    id,
                    who,
                    answer: Some(tx),
                    answered: false,
                    confirming: false,
                    stop: None,
                });
                hub.publish(&g);
                id
            };
            // The caller ends the question (`Pairing::end_question`) when the exchange is over,
            // whatever ends it, so the sheet keeps showing it while the merge is decided.
            question.store(id, Ordering::SeqCst);
            rx.await.unwrap_or(Decision::Cancelled)
        }
    }

    fn holdings(&self) -> Result<Holdings, SyncError> {
        self.hub.host.holdings()
    }

    fn reviewing(&self) -> impl Future<Output = ()> + Send {
        let (tx, rx) = oneshot::channel();
        {
            let mut g = self.hub.lock();
            let id = self.question();
            if let Some(p) = g.asking.as_mut().filter(|p| p.id == id) {
                p.confirming = true;
                p.stop = Some(tx);
                self.hub.publish(&g);
            }
        }
        // Resolves when the pairing is stopped here (the question is dropped with its sender).
        async move {
            let _ = rx.await;
        }
    }

    fn backup(&self) -> Result<(), SyncError> {
        self.hub.host.backup()
    }

    fn commit(&self, paired: &Paired) -> Result<Stored, SyncError> {
        self.hub.commit(paired, None)
    }
}
