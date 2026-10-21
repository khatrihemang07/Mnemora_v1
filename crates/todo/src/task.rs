//! The Task types and the rules that decide whether input is acceptable.

use crate::error::TodoError;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Longest Task title, in characters.
pub const MAX_TITLE_CHARS: usize = 500;

/// Where a Task stands. A Task is open until it is done or marked Won't do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Still to do.
    Open,
    /// Completed; `closed_at` says when.
    Done,
    /// Decided against; `closed_at` says when.
    Wont,
}

impl TaskState {
    /// The value stored in the `state` column.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Done => "done",
            Self::Wont => "wont",
        }
    }

    /// Reads a stored value; anything else is a corrupt row.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "open" => Some(Self::Open),
            "done" => Some(Self::Done),
            "wont" => Some(Self::Wont),
            _ => None,
        }
    }
}

// Millisecond instants are i64, which specta refuses to export. They fit a JS number (< 2^53), so
// they are typed `u32` for the bindings only: it makes TypeScript see a plain `number`.
/// One thing to do.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct Task {
    /// `UUIDv7`.
    pub id: String,
    /// The List it belongs to; none means the Inbox.
    pub list_id: Option<String>,
    /// One line, up to 500 characters.
    pub title: String,
    /// Markdown notes.
    pub notes: String,
    /// Due date, `YYYY-MM-DD`.
    pub due_date: Option<String>,
    /// Due time, `HH:MM`, floating local.
    pub due_time: Option<String>,
    /// Repeat unit: `day`, `week`, `month` or `year`.
    pub repeat_freq: Option<String>,
    /// Repeat interval, 1 to 99.
    pub repeat_every: Option<u32>,
    /// Weekday bitmask for weekly Repeats.
    pub repeat_days: Option<u32>,
    /// `fixed` (from the Due date) or `after` (from the day it was done).
    pub repeat_mode: Option<String>,
    /// 1 (highest) to 4 (none).
    pub priority: u32,
    /// Open, done or Won't do.
    pub state: TaskState,
    /// When it was done or marked Won't do: UTC milliseconds.
    #[specta(type = Option<u32>)]
    pub closed_at: Option<i64>,
    /// For an occurrence copy, the Task it was made from.
    pub repeat_of: Option<String>,
    /// What the natural-language parser recognised, as JSON.
    pub understood: Option<String>,
    /// Text that sorts bytewise into the manual order, shared by every Task.
    pub sort_key: String,
    /// Row creation time: UTC milliseconds.
    #[specta(type = u32)]
    pub created_at: i64,
    /// Row update time: UTC milliseconds.
    #[specta(type = u32)]
    pub updated_at: i64,
    /// How many live Comments it has: counted when read, never stored (and left out of the
    /// change-log snapshot).
    pub comment_count: u32,
}

/// The Inbox: open Tasks that belong to no List, in manual order.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct InboxView {
    /// Open Tasks, first to last.
    pub tasks: Vec<Task>,
}

/// A title that is allowed: something visible, one line, at most 500 characters. Returns it
/// trimmed.
pub fn validate_title(title: &str) -> Result<&str, TodoError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(TodoError::Validation("A Task needs a title"));
    }
    if title.contains(['\n', '\r']) {
        return Err(TodoError::Validation("A title must be a single line"));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(TodoError::Validation(
            "A title can be at most 500 characters",
        ));
    }
    Ok(title)
}

/// Lowest and highest Priority: 1 is the most urgent, 4 means none.
pub const PRIORITIES: std::ops::RangeInclusive<u32> = 1..=4;

/// A Priority from 1 to 4.
pub fn validate_priority(priority: u32) -> Result<u32, TodoError> {
    if PRIORITIES.contains(&priority) {
        Ok(priority)
    } else {
        Err(TodoError::Validation("Priority is 1 to 4"))
    }
}

/// Longest notes, in characters.
pub const MAX_NOTES_CHARS: usize = 100_000;

/// Notes that are allowed: any text, empty included, up to 100,000 characters.
pub fn validate_notes(notes: &str) -> Result<&str, TodoError> {
    if notes.chars().count() > MAX_NOTES_CHARS {
        return Err(TodoError::Validation(
            "Notes can be at most 100,000 characters",
        ));
    }
    Ok(notes)
}

/// Base-62 digits in byte order, so keys built from them sort the same as text and as numbers.
const DIGITS: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
/// Width of the keys made here.
const KEY_WIDTH: usize = 8;

/// The key for a Task that goes after every other: the first one sits mid-range so there is room
/// before it. Keys are text that sorts bytewise, like fractional-index keys, so the ordering
/// slice can insert between any two of these. A last key in some other shape gets a suffix,
/// which always sorts after it.
pub fn key_after(last: Option<&str>) -> String {
    let Some(last) = last else {
        return format!("{}{}", DIGITS[31] as char, "0".repeat(KEY_WIDTH - 1));
    };
    let mut bytes = last.as_bytes().to_vec();
    let counter = bytes.len() == KEY_WIDTH && bytes.iter().all(|b| DIGITS.contains(b));
    if counter {
        for b in bytes.iter_mut().rev() {
            let at = DIGITS.iter().position(|d| d == b).unwrap_or(0);
            if at + 1 < DIGITS.len() {
                *b = DIGITS[at + 1];
                return String::from_utf8_lossy(&bytes).into_owned();
            }
            *b = DIGITS[0];
        }
    }
    format!("{last}{}", DIGITS[31] as char)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_trimmed_single_line_and_bounded() {
        assert_eq!(validate_title("  Buy oat milk \t").unwrap(), "Buy oat milk");
        for bad in ["", "   ", "\n", "a\nb", "a\r\nb"] {
            assert!(validate_title(bad).is_err(), "{bad:?}");
        }
        assert!(validate_title(&"é".repeat(500)).is_ok());
        assert!(validate_title(&"é".repeat(501)).is_err());
    }

    #[test]
    fn keys_after_always_sort_later() {
        assert_eq!(key_after(None), "V0000000");
        let mut key = key_after(None);
        for _ in 0..200 {
            let next = key_after(Some(&key));
            assert!(next > key, "{next} should follow {key}");
            key = next;
        }
        assert_eq!(key_after(Some("0000000z")), "00000010");
        assert!(key_after(Some("zzzzzzzz")).as_str() > "zzzzzzzz");
        assert!(key_after(Some("a0V")).as_str() > "a0V");
    }
}
