//! Sync's own errors, mapped into the IPC `AppError` at the crate boundary.

use mnemora_core::error::{AppError, ErrorCode};

/// Why a Sync operation failed.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// Input broke a rule; the message is shown to the user.
    #[error("{0}")]
    Validation(&'static str),
}

impl From<SyncError> for AppError {
    fn from(e: SyncError) -> Self {
        match e {
            SyncError::Validation(_) => Self::new(ErrorCode::Validation, e.to_string()),
        }
    }
}
