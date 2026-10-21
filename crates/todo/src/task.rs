//! The Task types and the rules that decide whether input is acceptable.

use crate::arrange::{GroupBy, TaskGroup, TaskSort};
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

/// What a captured Task carries besides its title, List and When.
#[derive(Debug, Clone, Default, Deserialize, Type)]
pub struct NewTaskDetails {
    /// Markdown notes.
    #[serde(default)]
    pub notes: String,
    /// 1 to 4; none means 4.
    pub priority: Option<u32>,
    /// A Repeat; it needs the clock of `when` to find its first day.
    pub repeat: Option<crate::recurrence::RepeatRule>,
    /// What the parser recognised, as JSON `[{word, kind, value}]`.
    pub understood: Option<String>,
}

/// The most the record of understood words may hold, in characters.
const MAX_UNDERSTOOD_CHARS: usize = 4000;

/// The record of understood words: a JSON array, or nothing.
pub fn validate_understood(understood: Option<&str>) -> Result<Option<&str>, TodoError> {
    let Some(text) = understood else {
        return Ok(None);
    };
    let ok = text.chars().count() <= MAX_UNDERSTOOD_CHARS
        && serde_json::from_str::<Vec<serde_json::Value>>(text).is_ok();
    if !ok {
        return Err(TodoError::Validation(
            "That record of understood words is not valid",
        ));
    }
    Ok(Some(text))
}

/// The Inbox: open Tasks that belong to no List, in groups, then the undated Tasks that sit in a
/// List. Only the first part counts toward the badge.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct InboxView {
    /// How the Tasks with no List are grouped (a Preference).
    pub group_by: GroupBy,
    /// How they sort inside each group (a Preference).
    pub sort: TaskSort,
    /// Groups of Tasks with no List, then "No date, in a List" when there are such Tasks.
    pub groups: Vec<TaskGroup>,
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
}
