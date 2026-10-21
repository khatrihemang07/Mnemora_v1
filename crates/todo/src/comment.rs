//! Comments on a Task: the type and the rule that decides whether a body is acceptable.

use serde::Serialize;
use specta::Type;

use crate::error::TodoError;

/// Longest Comment, in characters.
pub const MAX_COMMENT_CHARS: usize = 5000;

/// A note left on a Task. It belongs to the Task row it was written on: for a repeating Task, the
/// current occurrence (completing moves it to the done copy).
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct Comment {
    /// `UUIDv7`.
    pub id: String,
    /// The Task it is on.
    pub task_id: String,
    /// Text with inline Markdown marks (line breaks allowed), up to 5000 characters.
    pub body: String,
    /// Whether it was changed after it was written.
    pub edited: bool,
    /// When it was written: UTC milliseconds.
    #[specta(type = u32)]
    pub created_at: i64,
    /// When it was last edited: UTC milliseconds.
    #[specta(type = u32)]
    pub updated_at: i64,
}

/// A body that is allowed: something visible, at most 5000 characters; line breaks are fine.
/// Returns it trimmed.
pub fn validate_body(body: &str) -> Result<&str, TodoError> {
    let body = body.trim();
    if body.is_empty() {
        return Err(TodoError::Validation("A comment can't be blank"));
    }
    if body.chars().count() > MAX_COMMENT_CHARS {
        return Err(TodoError::Validation(
            "A comment can be at most 5000 characters",
        ));
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bodies_are_trimmed_multi_line_and_bounded() {
        assert_eq!(validate_body("  hi \t").unwrap(), "hi");
        assert_eq!(validate_body("a\n\nb\n").unwrap(), "a\n\nb");
        for bad in ["", "  ", "\n\n"] {
            assert!(validate_body(bad).is_err(), "{bad:?}");
        }
        assert!(validate_body(&"é".repeat(5000)).is_ok());
        assert!(validate_body(&"é".repeat(5001)).is_err());
    }
}
