//! The rules for a question and for a Conversation's title.

use crate::error::AiError;
use crate::types::LocalContext;

/// Longest title a Conversation gets from its first question, in characters.
pub const TITLE_CHARS: usize = 80;

/// The title for a Conversation started by `question`: its whitespace collapsed to single spaces
/// and cut to [`TITLE_CHARS`] characters. No Model request is made for it.
pub fn title_of(question: &str) -> String {
    let collapsed = question.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
        .chars()
        .take(TITLE_CHARS)
        .collect::<String>()
        .trim_end()
        .to_owned()
}

/// Longest Saved question, in characters. A Saved question is a short prompt, not a document.
pub const SAVED_CHARS: usize = 500;

/// The whitespace of a title or question collapsed to single spaces, so it reads on one line.
pub fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A title the person typed: collapsed, not blank and at most [`TITLE_CHARS`] characters. Unlike
/// the first question's title, a typed one is refused rather than cut, so the person sees it.
pub fn validate_title(raw: &str) -> Result<String, AiError> {
    let title = collapse(raw);
    if title.is_empty() {
        return Err(AiError::Validation("Give this Conversation a title."));
    }
    if title.chars().count() > TITLE_CHARS {
        return Err(AiError::Validation("Keep the title to 80 characters."));
    }
    Ok(title)
}

/// A Saved question: the same rules as a question, collapsed, and at most [`SAVED_CHARS`].
pub fn validate_saved(raw: &str) -> Result<String, AiError> {
    validate_question(raw)?;
    let text = collapse(raw);
    if text.chars().count() > SAVED_CHARS {
        return Err(AiError::Validation(
            "Keep a Saved question to 500 characters.",
        ));
    }
    Ok(text)
}

/// A question must say something.
pub fn validate_question(text: &str) -> Result<(), AiError> {
    if text.trim().is_empty() {
        return Err(AiError::Validation("Type a question first."));
    }
    Ok(())
}

/// The calendar sent with a question must be usable: a date, a weekday, a time zone and a week start.
pub fn validate_local(local: &LocalContext) -> Result<(), AiError> {
    let date = local.today.as_bytes();
    let date_ok = date.len() == 10
        && date.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        });
    if !date_ok {
        return Err(AiError::Validation("The date on this device is not valid."));
    }
    if local.weekday.trim().is_empty() || local.time_zone.trim().is_empty() {
        return Err(AiError::Validation(
            "The time zone on this device is not known.",
        ));
    }
    if local.week_start != "mon" && local.week_start != "sun" {
        return Err(AiError::Validation("The week start is not valid."));
    }
    Ok(())
}
