//! The network [`Transport`](crate::Transport): TCP + TLS 1.3 between two devices that have
//! pinned each other's key (ADR 0017).
//!
//! Layers, bottom up:
//! - [`identity`]: this device's Ed25519 key (a 0600 file) and its self-signed certificate.
//! - [`tls`]: rustls configs whose verifiers accept exactly the keys in a [`Trust`] set, on both
//!   sides (mutual TLS). No certificate authority, no names, no expiry: the key is the identity.
//! - [`frame`]: JSON messages with a 4-byte big-endian length prefix, at most 8 MB.
//! - [`handshake`]: the first frame each way, and the rules that refuse a peer.
//! - [`link`]: the listener (port 47311, else an ephemeral one), the dialler, and
//!   [`NetTransport`], which bridges a socket to the non-blocking `Transport` with channels.
//!
//! Pairing (#62) plugs in below the handshake: [`secure_dial`] and the listener's
//! [`Incoming::Unpinned`] hand over a TLS [`Channel`] to a peer whose key is not pinned yet (only
//! while [`Trust::set_pairing_open`] is on), and #62 runs SPAKE2 over it with the same framing.
//! Revocation (#69) is [`Trust::unpin`].
//!
//! Tasks are spawned with `tokio::spawn`, so everything here must be called inside a Tokio
//! runtime (Tauri's in the app, a test runtime in tests). Never log message content.

pub mod frame;
pub mod handshake;
pub mod identity;
pub mod link;
pub mod tls;

#[cfg(test)]
mod tests;

use crate::error::SyncError;

pub use frame::{Channel, MAX_FRAME};
pub use handshake::{Handshake, PROTOCOL_VERSION, Refusal};
pub use identity::{DeviceIdentity, PublicKey};
pub use link::{
    DEFAULT_PORT, Established, Incoming, Listener, LocalHandshake, NetConfig, NetTransport,
    PeerInfo, Probed, Secured, TlsChannel, dial, probe, secure_dial,
};
pub use tls::Trust;

/// Why a network operation failed.
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    /// The socket failed.
    #[error("network error: {0}")]
    Io(#[from] std::io::Error),
    /// TLS refused the connection (an unpinned key, a bad certificate, a protocol error).
    #[error("TLS refused the connection: {0}")]
    Tls(String),
    /// A frame announced or held more than [`MAX_FRAME`] bytes; the connection is closed.
    #[error("a message of {0} bytes is over the {MAX_FRAME} byte limit")]
    TooLarge(usize),
    /// The other end closed the connection.
    #[error("the connection was closed")]
    Closed,
    /// A frame was not the JSON it should be.
    #[error("malformed message: {0}")]
    Malformed(String),
    /// The handshake refused the peer.
    #[error("peer refused: {0}")]
    Refused(Refusal),
    /// Nothing happened in time.
    #[error("timed out")]
    Timeout,
    /// This device's own handshake could not be built.
    #[error("could not build this device's handshake: {0}")]
    Local(String),
    /// This device's key file could not be read or made.
    #[error("device key: {0}")]
    Key(String),
}

impl From<NetError> for SyncError {
    fn from(e: NetError) -> Self {
        match e {
            NetError::Io(_) | NetError::Closed | NetError::Timeout => Self::Disconnected,
            other => Self::Protocol(other.to_string()),
        }
    }
}
