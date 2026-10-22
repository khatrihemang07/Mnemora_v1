//! State the supervisor and the connection tasks share.

use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::Host;
use super::status::PeerLive;

/// Progress is announced at most this often, so a big sync does not flood the window.
const PROGRESS_EVERY: Duration = Duration::from_millis(120);

pub(super) struct Shared {
    pub host: Arc<dyn Host>,
    /// This device's id.
    pub me: String,
    live: Mutex<HashMap<String, PeerLive>>,
    /// Which paired devices have a held connection, and whether this device dialled it.
    dialled: Mutex<HashMap<String, bool>>,
    /// Sessions run since this started.
    pub sessions: AtomicUsize,
    /// Tasks the supervisor owns right now (itself included).
    pub tasks: AtomicUsize,
    last_announced: Mutex<Option<Instant>>,
}

impl Shared {
    pub fn new(host: Arc<dyn Host>, me: String) -> Self {
        Self {
            host,
            me,
            live: Mutex::default(),
            dialled: Mutex::default(),
            sessions: AtomicUsize::new(0),
            tasks: AtomicUsize::new(0),
            last_announced: Mutex::new(None),
        }
    }

    /// Changes one device's live state and tells the window. Progress-only changes (`force` off)
    /// are throttled; the window catches up on the next change that is forced.
    pub fn update(&self, peer: &str, force: bool, f: impl FnOnce(&mut PeerLive)) {
        {
            let mut live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
            f(live.entry(peer.to_owned()).or_default());
        }
        let mut last = self
            .last_announced
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if force || last.is_none_or(|at| at.elapsed() >= PROGRESS_EVERY) {
            *last = Some(Instant::now());
            drop(last);
            self.host.status_changed();
        }
    }

    /// Drops everything known about a device (it was forgotten).
    pub fn remove(&self, peer: &str) {
        self.live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(peer);
        self.set_dialled(peer, None);
        self.host.status_changed();
    }

    pub fn snapshot(&self) -> HashMap<String, PeerLive> {
        self.live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn set_dialled(&self, peer: &str, by_me: Option<bool>) {
        let mut map = self.dialled.lock().unwrap_or_else(PoisonError::into_inner);
        match by_me {
            Some(by_me) => {
                map.insert(peer.to_owned(), by_me);
            }
            None => {
                map.remove(peer);
            }
        }
    }

    pub fn dialled(&self, peer: &str) -> Option<bool> {
        self.dialled
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(peer)
            .copied()
    }
}
