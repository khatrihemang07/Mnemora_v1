//! The Journal's own errors, mapped into the IPC `AppError` at the crate boundary.

use mnemora_core::error::{AppError, ErrorCode};

/// Why a Journal operation failed.
#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    /// Input broke a rule; the message is shown to the user.
    #[error("{0}")]
    Validation(&'static str),
    /// The Entry does not exist (or is deleted).
    #[error("That Entry no longer exists")]
    EntryNotFound,
    /// The change-log snapshot could not be built.
    #[error("Could not record the change")]
    Snapshot(#[source] serde_json::Error),
}

impl From<JournalError> for AppError {
    fn from(e: JournalError) -> Self {
        let code = match &e {
            JournalError::Validation(_) => ErrorCode::Validation,
            JournalError::EntryNotFound => ErrorCode::UnknownKey,
            JournalError::Snapshot(_) => ErrorCode::Internal,
        };
        let error = Self::new(code, e.to_string());
        match e {
            JournalError::Snapshot(source) => error.with_details(source),
            _ => error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_to_the_wire_codes_the_ui_relies_on() {
        let v: AppError = JournalError::Validation("Not a valid date").into();
        assert_eq!(
            (v.code, v.message.as_str()),
            (ErrorCode::Validation, "Not a valid date")
        );
        let n: AppError = JournalError::EntryNotFound.into();
        assert_eq!(n.code, ErrorCode::UnknownKey);
        let s: AppError =
            JournalError::Snapshot(serde_json::from_str::<u8>("x").unwrap_err()).into();
        assert_eq!(s.code, ErrorCode::Internal);
        assert!(s.details.is_some());
    }
}
