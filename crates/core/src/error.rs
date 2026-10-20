//! The single error type that crosses the IPC boundary.

use serde::Serialize;
use specta::Type;

/// Machine-readable failure category the UI can branch on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// A value failed validation.
    Validation,
    /// A key or id is not registered or does not exist.
    UnknownKey,
    /// Filesystem failure.
    Io,
    /// Database failure.
    Database,
    /// Anything else; a bug or an unexpected state.
    Internal,
}

/// Error returned from commands: `{code, message, details}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct AppError {
    /// Category for UI branching.
    pub code: ErrorCode,
    /// Short, user-presentable summary.
    pub message: String,
    /// Technical detail for "copy details"; never shown by default.
    pub details: Option<String>,
}

impl AppError {
    /// Builds an error with no details.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    /// Attaches technical details.
    #[must_use]
    pub fn with_details(mut self, details: impl ToString) -> Self {
        self.details = Some(details.to_string());
        self
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        Self::new(ErrorCode::Database, "Database error").with_details(e)
    }
}

impl From<rusqlite_migration::Error> for AppError {
    fn from(e: rusqlite_migration::Error) -> Self {
        Self::new(ErrorCode::Database, "Database migration failed").with_details(e)
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::new(ErrorCode::Io, "File system error").with_details(e)
    }
}

/// Convenience alias used across the workspace.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_wire_shape() {
        let e = AppError::new(ErrorCode::UnknownKey, "no such key").with_details("core.nope");
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"code": "unknown_key", "message": "no such key", "details": "core.nope"})
        );
    }

    #[test]
    fn details_default_to_null() {
        let v = serde_json::to_value(AppError::new(ErrorCode::Internal, "x")).unwrap();
        assert_eq!(v["details"], serde_json::Value::Null);
    }

    #[test]
    fn io_errors_map_to_io_code() {
        let e: AppError = std::io::Error::other("disk").into();
        assert_eq!(e.code, ErrorCode::Io);
        assert_eq!(e.details.as_deref(), Some("disk"));
    }

    #[test]
    fn sqlite_errors_map_to_database_code() {
        let e: AppError = rusqlite::Error::QueryReturnedNoRows.into();
        assert_eq!(e.code, ErrorCode::Database);
    }
}
