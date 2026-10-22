//! Pairing two devices (spec #54, "Pairing and trust"). One device shows an 8-digit code, the
//! other types it; SPAKE2 turns the code into a key that authenticates the exchange of public
//! keys over the TLS connection; the person on the showing device allows or denies; then each
//! pins the other.
//!
//! - [`code`]: the code itself.
//! - [`msg`]: the messages on the wire.
//! - [`proto`]: the exchange from each end, over any framed channel.
//! - [`hub`]: [`Pairing`], the state the UI drives (show, enter, allow, cancel).
//! - [`host`]: what the app provides (this device's facts, storage, events).
//! - [`view`]: what the pairing sheet renders.

use std::time::Duration;

pub mod code;
pub mod host;
pub mod hub;
pub mod msg;
pub(crate) mod proto;
#[cfg(test)]
mod tests;
pub mod view;

pub use host::{Paired, PairingHost, Stored};
pub use hub::Pairing;
pub use view::{Older, PairingError, PairingView, Platform};

/// How long each part of a pairing may take. The defaults are the app's; tests shorten them.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// How long a code (and the Allow question after it) lives.
    pub code_life: Duration,
    /// How long the typing device looks for a device showing a code.
    pub search: Duration,
    /// How long one reply may take during the exchange (not counting the person's answer).
    pub step: Duration,
    /// Extra time the typing device waits beyond the code's life before giving up.
    pub grace: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            code_life: code::LIFE,
            search: Duration::from_secs(5),
            step: Duration::from_secs(10),
            grace: Duration::from_secs(5),
        }
    }
}
