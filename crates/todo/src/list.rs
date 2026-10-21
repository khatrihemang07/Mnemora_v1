//! The List types and the rules that decide whether a name or emoji is acceptable.

use mnemora_core::text::fold;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::TodoError;
use crate::task::Task;

/// Longest List name, in characters.
pub const MAX_LIST_NAME_CHARS: usize = 60;
/// Longest emoji, in characters: room for a family or flag sequence.
pub const MAX_EMOJI_CHARS: usize = 16;

// Millisecond instants are typed `u32` for the bindings only, like on `Task`.
/// A flat group of Tasks. Tasks with no List are in the Inbox, which is not a List.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct TodoList {
    /// `UUIDv7`.
    pub id: String,
    /// Unique among Lists once case and accents are ignored; up to 60 characters.
    pub name: String,
    /// One emoji shown beside the name; none shows a neutral list icon.
    pub emoji: Option<String>,
    /// Text that sorts bytewise into the List order.
    pub sort_key: String,
    /// Row creation time: UTC milliseconds.
    #[specta(type = u32)]
    pub created_at: i64,
    /// Row update time: UTC milliseconds.
    #[specta(type = u32)]
    pub updated_at: i64,
}

/// A List with its Task counts.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct ListEntry {
    /// The List.
    pub list: TodoList,
    /// Open Tasks in it.
    pub open_count: u32,
    /// Every Task in it, done ones too: what deleting the List has to deal with.
    pub task_count: u32,
}

/// What the sidebar shows: the Inbox count and every List, first to last.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct SidebarView {
    /// Open Tasks with no List.
    pub inbox_count: u32,
    /// Open Tasks due today, plus every Overdue one: Today's count.
    pub today_count: u32,
    /// Open Tasks due before today: the Today count turns red while this is above zero.
    pub overdue_count: u32,
    /// Lists in their order.
    pub lists: Vec<ListEntry>,
}

/// One List's page: the List and its open Tasks in manual order.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct ListView {
    /// The List.
    pub list: TodoList,
    /// Its open Tasks.
    pub tasks: Vec<Task>,
}

/// Open Tasks of one List (or the Inbox) inside the All view.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct AllGroup {
    /// The List; none is the Inbox.
    pub list: Option<TodoList>,
    /// Its open Tasks in manual order.
    pub tasks: Vec<Task>,
}

/// The All view: open Tasks grouped by List, the Inbox first. Empty groups are left out.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct AllView {
    /// Groups, Inbox first, then Lists in order.
    pub groups: Vec<AllGroup>,
}

/// What happens to a deleted List's Tasks.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeleteChoice {
    /// Delete them with the List (the default).
    DeleteTasks,
    /// Move them to the Inbox.
    MoveToInbox,
    /// Move them to another List.
    MoveToList {
        /// The receiving List.
        list_id: String,
    },
}

/// What a List deletion did: hand it back to `todo_list_restore` to undo it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ListDeletion {
    /// The deleted List.
    pub list_id: String,
    /// Tasks that were deleted with it.
    pub deleted_task_ids: Vec<String>,
    /// Tasks that were moved out of it.
    pub moved_task_ids: Vec<String>,
}

/// A List name that is allowed: something visible, one line, at most 60 characters. Returns it
/// trimmed.
pub fn validate_list_name(name: &str) -> Result<&str, TodoError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(TodoError::Validation("A List needs a name"));
    }
    if name.contains(['\n', '\r']) {
        return Err(TodoError::Validation("A List name must be a single line"));
    }
    if name.chars().count() > MAX_LIST_NAME_CHARS {
        return Err(TodoError::Validation(
            "A List name can be at most 60 characters",
        ));
    }
    Ok(name)
}

/// The name with case and accents ignored: what uniqueness compares.
pub fn name_key(name: &str) -> String {
    fold(name)
}

/// An emoji that is allowed: a few characters with no spaces or letters. Blank means none.
pub fn validate_emoji(emoji: Option<&str>) -> Result<Option<&str>, TodoError> {
    let Some(emoji) = emoji.map(str::trim).filter(|e| !e.is_empty()) else {
        return Ok(None);
    };
    if emoji.chars().count() > MAX_EMOJI_CHARS
        || emoji
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c.is_ascii_alphabetic())
    {
        return Err(TodoError::Validation("That doesn't look like an emoji"));
    }
    Ok(Some(emoji))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_single_line_and_bounded() {
        assert_eq!(validate_list_name("  Home \t").unwrap(), "Home");
        for bad in ["", "  ", "a\nb"] {
            assert!(validate_list_name(bad).is_err(), "{bad:?}");
        }
        assert!(validate_list_name(&"é".repeat(60)).is_ok());
        assert!(validate_list_name(&"é".repeat(61)).is_err());
    }

    #[test]
    fn name_keys_ignore_case_and_accents() {
        assert_eq!(name_key("Café"), name_key("  cafe".trim()));
        assert_ne!(name_key("Work"), name_key("Home"));
    }

    #[test]
    fn emoji_are_short_and_have_no_letters_or_spaces() {
        assert_eq!(validate_emoji(Some(" 🏠 ")).unwrap(), Some("🏠"));
        assert_eq!(validate_emoji(Some("  ")).unwrap(), None);
        assert_eq!(validate_emoji(None).unwrap(), None);
        assert!(validate_emoji(Some("👨‍👩‍👧")).is_ok());
        for bad in ["ab", "🏠 🏠", &"🏠".repeat(17)] {
            assert!(validate_emoji(Some(bad)).is_err(), "{bad:?}");
        }
    }
}
