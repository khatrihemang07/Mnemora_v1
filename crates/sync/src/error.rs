//! Sync's own errors, mapped into the IPC `AppError` at the crate boundary.

use mnemora_core::error::{AppError, ErrorCode};

/// Why a Sync operation failed.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// Input broke a rule; the message is shown to the user.
    #[error("{0}")]
    Validation(&'static str),
    /// Sync is off on this device, so no session may start.
    #[error("Sync is off on this device")]
    Off,
    /// Sync is on but its network side has not started yet.
    #[error("Sync is still starting. Try again in a moment")]
    NotReady,
    /// The other device went away; what was applied so far stays, and the next session resumes.
    #[error("The connection to the other device was lost")]
    Disconnected,
    /// The other device sent something this one cannot make sense of.
    #[error("Unexpected message from the other device: {0}")]
    Protocol(String),
    /// A record is stamped more than five minutes in the future of this device's clock; nothing
    /// of the exchange is stored (see `pause`).
    #[error("The other device’s clock is too far ahead")]
    ClockAhead {
        /// How long until the newest such record is no more than five minutes ahead, in
        /// milliseconds: when the exchange can be tried again.
        resumes_in_ms: i64,
    },
    /// A record of a kind no Module here syncs.
    #[error("The other device sent a kind of record this device does not know")]
    UnknownEntity,
    /// The database or a Module refused.
    #[error(transparent)]
    App(#[from] AppError),
}

impl From<rusqlite::Error> for SyncError {
    fn from(e: rusqlite::Error) -> Self {
        Self::App(e.into())
    }
}

impl From<SyncError> for AppError {
    fn from(e: SyncError) -> Self {
        match e {
            SyncError::Validation(_) | SyncError::Off | SyncError::NotReady => {
                Self::new(ErrorCode::Validation, e.to_string())
            }
            SyncError::Disconnected | SyncError::ClockAhead { .. } => {
                Self::new(ErrorCode::Io, e.to_string())
            }
            SyncError::Protocol(ref why) => {
                Self::new(ErrorCode::Internal, "Sync got an unexpected message").with_details(why)
            }
            SyncError::UnknownEntity => Self::new(ErrorCode::Internal, e.to_string()),
            SyncError::App(app) => app,
        }
    }
}
