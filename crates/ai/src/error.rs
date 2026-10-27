//! The AI Module's own errors, mapped into the IPC `AppError` at the crate boundary.

use mnemora_core::error::{AppError, ErrorCode};

/// Why an AI operation failed.
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    /// Input broke a rule; the message is shown to the person.
    #[error("{0}")]
    Validation(&'static str),
    /// The Conversation does not exist (or is deleted).
    #[error("That Conversation no longer exists")]
    NotFound,
    /// The Provider does not exist (or is deleted).
    #[error("That Provider no longer exists")]
    ProviderNotFound,
    /// A stored entry could not be read or written.
    #[error("Could not read or store the Conversation")]
    Payload(#[source] serde_json::Error),
}

impl From<AiError> for AppError {
    fn from(e: AiError) -> Self {
        let code = match &e {
            AiError::Validation(_) => ErrorCode::Validation,
            AiError::NotFound | AiError::ProviderNotFound => ErrorCode::NotFound,
            AiError::Payload(_) => ErrorCode::Internal,
        };
        let error = Self::new(code, e.to_string());
        match e {
            AiError::Payload(source) => error.with_details(source),
            _ => error,
        }
    }
}
