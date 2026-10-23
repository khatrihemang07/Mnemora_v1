//! What Sync has running. Nothing, unless Sync is on: this is the one place a listener, a
//! discovery advertisement or a background task may live, so "inert when off" is a property of
//! this type and can be asserted through [`SyncRuntime::probe`].

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};

use mnemora_core::events::EventBus;
use mnemora_core::hlc;
use rusqlite::Connection;
use tokio::sync::Notify;

use crate::error::SyncError;
use crate::live::PeerLive;
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
    /// Signalled when this device records a change of its own. It outlives the services: a change
    /// made while Sync is off is pushed as soon as it turns on.
    local_changes: Arc<Notify>,
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

    /// The signal a write to the change log raises; the session manager waits on it.
    pub fn local_changes(&self) -> Arc<Notify> {
        Arc::clone(&self.local_changes)
    }

    fn with_network<R>(&self, f: impl FnOnce(&Network) -> R) -> Option<R> {
        let services = self.services.lock().unwrap_or_else(PoisonError::into_inner);
        services.as_ref()?.network.as_ref().map(f)
    }

    /// "Sync now": every reachable paired device is synced now, whatever "Sync automatically" says.
    /// Does nothing while Sync is off or still starting.
    pub fn sync_now(&self) {
        self.with_network(|n| n.live().sync_now());
    }

    /// The network changed or the app came back: unreachable devices are tried again from the
    /// start of the backoff.
    pub fn resume(&self) {
        self.with_network(|n| n.live().resume());
    }

    /// The set of paired devices changed (a pairing, or leaving the group).
    pub fn peers_changed(&self) {
        self.with_network(|n| n.live().refresh_peers());
    }

    /// "Sync now" for one device.
    pub fn sync_now_with(&self, device: &str) {
        self.with_network(|n| n.live().sync_now_with(device));
    }

    /// Stops trusting one key at once (a device was forgotten here).
    pub fn unpin(&self, key: &crate::net::PublicKey) {
        self.with_network(|n| n.unpin(key));
    }

    /// What a connection needs, once the network side has started.
    pub fn net_config(&self) -> Option<crate::net::NetConfig> {
        self.with_network(Network::config)
    }

    /// "Sync automatically" was switched.
    pub fn options_changed(&self) {
        self.with_network(|n| n.live().options_changed());
    }

    /// What is happening with each paired device; empty while Sync is off.
    pub fn peers_live(&self) -> HashMap<String, PeerLive> {
        self.with_network(Network::peers_live).unwrap_or_default()
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
                sessions: s.sessions + s.network.as_ref().map_or(0, Network::sessions),
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
