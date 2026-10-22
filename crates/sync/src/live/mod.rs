//! Live syncing: the session manager. While Sync is on it keeps one connection per reachable
//! paired device, runs a [`Session`](crate::Session) over it when it opens and again whenever
//! this device has new changes (3 s after the last one), dials devices it finds or remembers,
//! and backs off when they are not there.
//!
//! Everything runs as Tokio tasks owned by one supervisor ([`Live`]); dropping the [`Live`]
//! stops the dialler, every connection and every timer. Database work goes through the
//! [`Host`] on blocking threads, one short transaction per batch, so the app is never held up
//! by a big sync.

mod backoff;
mod conn;
mod dial;
mod manager;
mod shared;
mod status;
mod tie;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::Duration;

use mnemora_core::events::EventBus;
use rusqlite::Connection;
use tokio::sync::Notify;

use crate::error::SyncError;
use crate::pair::PairingHost;
use crate::registry::Registry;

pub use backoff::Schedule;
pub use manager::Live;
pub use status::{PeerLive, Progress};

/// What live syncing needs from the app around it, besides what pairing needs.
pub trait Host: PairingHost {
    /// Runs `f` on a database connection. The app gives sync its own connection to the same
    /// file, so a sync never waits behind a command and a command never waits behind a sync.
    fn with_db(
        &self,
        f: &mut dyn FnMut(&Connection) -> Result<(), SyncError>,
    ) -> Result<(), SyncError>;
    /// Every Module that syncs.
    fn registry(&self) -> Registry;
    /// Where applied changes are announced.
    fn bus(&self) -> EventBus;
    /// "Sync automatically" is on.
    fn automatic(&self) -> bool;
    /// Signalled whenever this device records a change of its own.
    fn local_changes(&self) -> Arc<Notify>;
    /// A paired device's status (or progress) changed: tell the window.
    fn status_changed(&self);
}

/// Runs `f` on the host's database.
pub(crate) fn with_db<R>(
    host: &dyn Host,
    f: impl FnOnce(&Connection) -> Result<R, SyncError>,
) -> Result<R, SyncError> {
    let mut f = Some(f);
    let mut out = None;
    host.with_db(&mut |conn| {
        if let Some(f) = f.take() {
            out = Some(f(conn)?);
        }
        Ok(())
    })?;
    out.ok_or_else(|| SyncError::Protocol("the database did not run the request".into()))
}

/// Runs `f` on the host's database on a blocking thread, so a slow transaction never stalls the
/// async tasks around it.
pub(crate) async fn blocking<R: Send + 'static>(
    host: &Arc<dyn Host>,
    f: impl FnOnce(&Connection) -> Result<R, SyncError> + Send + 'static,
) -> Result<R, SyncError> {
    let host = Arc::clone(host);
    tokio::task::spawn_blocking(move || with_db(&*host, f))
        .await
        .map_err(|e| SyncError::Protocol(format!("a database task failed: {e}")))?
}

/// The clock of live syncing, in one place so tests can speed it up.
#[derive(Debug, Clone)]
pub struct Pace {
    /// Quiet time after the last local change before changes are pushed.
    pub debounce: Duration,
    /// How long each address gets when dialling a device.
    pub dial_each: Duration,
    /// How long to wait between failed attempts to reach a device.
    pub retry: Schedule,
}

impl Default for Pace {
    fn default() -> Self {
        Self {
            debounce: Duration::from_secs(3),
            dial_each: Duration::from_secs(3),
            retry: Schedule::default(),
        }
    }
}

#[cfg(test)]
mod pace_tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_specified_ones() {
        let pace = Pace::default();
        assert_eq!(pace.debounce, Duration::from_secs(3));
        assert_eq!(pace.dial_each, Duration::from_secs(3));
        assert_eq!(pace.retry, Schedule::default());
    }
}
