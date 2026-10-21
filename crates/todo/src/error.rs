//! The Todo Module's own errors, mapped into the IPC `AppError` at the crate boundary.

use mnemora_core::error::{AppError, ErrorCode};

/// Why a Todo operation failed.
#[derive(Debug, thiserror::Error)]
pub enum TodoError {
    /// Input broke a rule; the message is shown to the user.
    #[error("{0}")]
    Validation(&'static str),
    /// Another live List has this name (ignoring case and accents). The message names the List as
    /// the user typed it, so it is shown but never logged (ADR 0008: no user text in logs).
    #[error("There’s already a List called {0}")]
    NameTaken(String),
    /// The Task does not exist (or is deleted).
    #[error("That Task no longer exists")]
    TaskNotFound,
    /// The List does not exist (or is deleted).
    #[error("That List no longer exists")]
    ListNotFound,
    /// The Comment does not exist (or is deleted).
    #[error("That comment no longer exists")]
    CommentNotFound,
    /// The change-log snapshot could not be built.
    #[error("Could not record the change")]
    Snapshot(#[source] serde_json::Error),
}

impl From<TodoError> for AppError {
    fn from(e: TodoError) -> Self {
        let code = match &e {
            TodoError::Validation(_) | TodoError::NameTaken(_) => ErrorCode::Validation,
            TodoError::TaskNotFound | TodoError::ListNotFound | TodoError::CommentNotFound => {
                ErrorCode::NotFound
            }
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
        let invalid: AppError = TodoError::Validation("A Task needs a title").into();
        assert_eq!(
            (invalid.code, invalid.message.as_str()),
            (ErrorCode::Validation, "A Task needs a title")
        );
        let taken: AppError = TodoError::NameTaken("Work".into()).into();
        assert_eq!(
            (taken.code, taken.message.as_str()),
            (ErrorCode::Validation, "There’s already a List called Work")
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
