//! What Sync has running. Nothing, unless Sync is on: this is the one place a listener, a
//! discovery advertisement or a background task may live, so "inert when off" is a property of
//! this type and can be asserted through [`SyncRuntime::probe`].

use std::net::SocketAddr;
use std::sync::{Mutex, PoisonError};

/// Everything that exists only while Sync is on. Later tickets add the TLS listener, the mDNS
/// advertisement and the session tasks here, so each has exactly one owner that [`SyncRuntime`]
/// drops when Sync turns off.
#[derive(Default)]
struct Services {
    /// Sockets accepting peers.
    listeners: Vec<SocketAddr>,
    /// Service names advertised on the local network.
    advertisements: Vec<String>,
    /// Background tasks (sessions, retries).
    tasks: usize,
}

/// A read-only view of [`SyncRuntime`], for tests and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProbe {
    /// Sync's services have been started.
    pub running: bool,
    /// Open listening sockets.
    pub listeners: usize,
    /// Active mDNS advertisements and browses.
    pub advertisements: usize,
    /// Live background tasks.
    pub tasks: usize,
}

/// Starts and stops Sync's services to match the Device setting.
#[derive(Default)]
pub struct SyncRuntime {
    services: Mutex<Option<Services>>,
}

impl SyncRuntime {
    /// A runtime with nothing started.
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes the running state match `enabled`: starts the services when Sync is on and none
    /// run, drops them all when it is off. Does nothing when already in that state, so it is
    /// safe to call at startup and after every change.
    pub fn reconcile(&self, enabled: bool) {
        let mut services = self.services.lock().unwrap_or_else(PoisonError::into_inner);
        match (enabled, services.is_some()) {
            (true, false) => {
                *services = Some(Services::default());
                tracing::info!(target: "sync", "started");
            }
            (false, true) => {
                *services = None;
                tracing::info!(target: "sync", "stopped");
            }
            _ => {}
        }
    }

    /// What is running right now.
    pub fn probe(&self) -> RuntimeProbe {
        let services = self.services.lock().unwrap_or_else(PoisonError::into_inner);
        services.as_ref().map_or(
            RuntimeProbe {
                running: false,
                listeners: 0,
                advertisements: 0,
                tasks: 0,
            },
            |s| RuntimeProbe {
                running: true,
                listeners: s.listeners.len(),
                advertisements: s.advertisements.len(),
                tasks: s.tasks,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_runtime_has_nothing_running() {
        let rt = SyncRuntime::new();
        assert_eq!(
            rt.probe(),
            RuntimeProbe {
                running: false,
                listeners: 0,
                advertisements: 0,
                tasks: 0
            }
        );
    }

    #[test]
    fn reconcile_starts_and_stops_and_is_idempotent() {
        let rt = SyncRuntime::new();
        rt.reconcile(false);
        assert!(!rt.probe().running);
        rt.reconcile(true);
        rt.reconcile(true);
        assert!(rt.probe().running);
        rt.reconcile(false);
        assert!(!rt.probe().running);
    }
}
