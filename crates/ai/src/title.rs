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
