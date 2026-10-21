//! The Todo Module's own errors, mapped into the IPC `AppError` at the crate boundary.

use mnemora_core::error::{AppError, ErrorCode};

/// Why a Todo operation failed.
#[derive(Debug, thiserror::Error)]
pub enum TodoError {
    /// Input broke a rule; the message is shown to the user.
    #[error("{0}")]
    Validation(&'static str),
    /// The Task does not exist (or is deleted).
    #[error("That Task no longer exists")]
    TaskNotFound,
    /// The List does not exist (or is deleted).
    #[error("That List no longer exists")]
    ListNotFound,
    /// The change-log snapshot could not be built.
    #[error("Could not record the change")]
    Snapshot(#[source] serde_json::Error),
}

impl From<TodoError> for AppError {
    fn from(e: TodoError) -> Self {
        let code = match &e {
            TodoError::Validation(_) => ErrorCode::Validation,
            TodoError::TaskNotFound | TodoError::ListNotFound => ErrorCode::NotFound,
            TodoError::Snapshot(_) => ErrorCode::Internal,
        };
        let error = Self::new(code, e.to_string());
        match e {
            TodoError::Snapshot(source) => error.with_details(source),
            _ => error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_to_the_wire_codes_the_ui_relies_on() {
        let v: AppError = TodoError::Validation("A Task needs a title").into();
        assert_eq!(
            (v.code, v.message.as_str()),
            (ErrorCode::Validation, "A Task needs a title")
        );
        let n: AppError = TodoError::TaskNotFound.into();
        assert_eq!(n.code, ErrorCode::NotFound);
        let l: AppError = TodoError::ListNotFound.into();
        assert_eq!(l.code, ErrorCode::NotFound);
        let s: AppError = TodoError::Snapshot(serde_json::from_str::<u8>("x").unwrap_err()).into();
        assert_eq!(s.code, ErrorCode::Internal);
        assert!(s.details.is_some());
    }
}
