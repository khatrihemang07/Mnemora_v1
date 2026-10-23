//! What the manager knows about each device right now, shared with whoever draws the Devices tab.

use serde::Serialize;
use specta::Type;

use crate::pause::Pause;

/// How far a running session has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct Progress {
    /// Changes moved so far, in either direction.
    pub done: u32,
    /// Changes the session will move.
    pub total: u32,
}

/// A device's live state. Absent from the manager's table means not reachable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PeerLive {
    /// A connection is open (or was, and ended on purpose).
    pub reachable: bool,
    /// A session is running over it.
    pub syncing: bool,
    /// Progress of that session.
    pub progress: Option<Progress>,
    /// Why sync with it is paused (its data versions or its clock), if it is.
    pub paused: Option<Pause>,
}
