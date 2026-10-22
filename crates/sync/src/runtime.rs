//! What Sync has running. Nothing, unless Sync is on: this is the one place a listener, a
//! discovery advertisement or a background task may live, so "inert when off" is a property of
//! this type and can be asserted through [`SyncRuntime::probe`].

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};

use mnemora_core::events::EventBus;
use mnemora_core::hlc;
use rusqlite::Connection;

use crate::error::SyncError;
use crate::network::Network;
use crate::pair::Pairing;
use crate::registry::Registry;
use crate::session::Session;

/// Everything that exists only while Sync is on. The network side (the TLS listener, the
/// discovery announcement, pairing and their tasks) is one [`Network`], attached once it has
/// started, so each has exactly one owner that [`SyncRuntime`] drops when Sync turns off.
#[derive(Default)]
struct Services {
    network: Option<Network>,
    /// Sessions started since Sync was switched on.
    sessions: usize,
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
    /// Sessions the engine has run since Sync was switched on. Zero while it is off.
    pub sessions: usize,
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
                sessions: 0,
            },
            |s| RuntimeProbe {
                running: true,
                listeners: usize::from(s.network.is_some()),
                advertisements: s.network.as_ref().map_or(0, Network::discovery_active),
                tasks: s.network.as_ref().map_or(0, Network::tasks),
                sessions: s.sessions,
            },
        )
    }

    /// Hands over the started network side. If Sync was switched off while it was starting, there
    /// is nothing to own it and it is given back to be dropped.
    pub fn attach(&self, network: Network) -> Result<(), Network> {
        let mut services = self.services.lock().unwrap_or_else(PoisonError::into_inner);
        match services.as_mut() {
            Some(s) if s.network.is_none() => {
                s.network = Some(network);
                Ok(())
            }
            _ => Err(network),
        }
    }

    /// Pairing, once Sync is on and its network side has started.
    pub fn pairing(&self) -> Result<Arc<Pairing>, SyncError> {
        let services = self.services.lock().unwrap_or_else(PoisonError::into_inner);
        let services = services.as_ref().ok_or(SyncError::Off)?;
        let network = services.network.as_ref().ok_or(SyncError::NotReady)?;
        Ok(Arc::clone(network.pairing()))
    }

    /// The listening address, once started.
    pub fn listening(&self) -> Option<SocketAddr> {
        let services = self.services.lock().unwrap_or_else(PoisonError::into_inner);
        services.as_ref()?.network.as_ref().map(Network::addr)
    }

    /// Announces this device again (a setting or the group changed).
    pub fn refresh_advert(&self) {
        if let Ok(pairing) = self.pairing() {
            pairing.refresh_advert();
        }
    }

    /// Stops trusting every key (this device left its group).
    pub fn unpin_all(&self) {
        let services = self.services.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(network) = services.as_ref().and_then(|s| s.network.as_ref()) {
            network.unpin_all();
        }
    }

    /// Starts an exchange with another device over `conn`, this device's database. Refuses
    /// while Sync is off: the engine does no work, not even reading the change log, unless
    /// the runtime is running.
    pub fn session(
        &self,
        conn: &Connection,
        registry: Registry,
        bus: EventBus,
    ) -> Result<Session, SyncError> {
        let mut services = self.services.lock().unwrap_or_else(PoisonError::into_inner);
        let services = services.as_mut().ok_or(SyncError::Off)?;
        let me = hlc::device_id(conn)?;
        services.sessions += 1;
        Ok(Session::new(me, registry, bus))
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
                tasks: 0,
                sessions: 0
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
