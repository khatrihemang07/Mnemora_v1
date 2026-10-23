//! The first-merge preview: what two devices hold, and which Lists will become one. Pure: the
//! counts come from the Modules (`Holdings`) and travel as they are.

use mnemora_core::syncable::Holdings;
use mnemora_core::text::fold;
use serde::{Deserialize, Serialize};
use specta::Type;

/// The numbers of one side of the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Counts {
    /// Journal Entries.
    pub entries: u32,
    /// Tasks.
    pub tasks: u32,
    /// Lists.
    pub lists: u32,
}

impl Counts {
    /// The counts of what a device holds.
    pub fn of(held: &Holdings) -> Self {
        Self {
            entries: held.entries,
            tasks: held.tasks,
            lists: u32::try_from(held.lists.len()).unwrap_or(u32::MAX),
        }
    }
}

/// Whether a first merge needs the preview: both devices hold something. When one is empty
/// nothing can surprise anyone, so the merge goes straight ahead.
pub fn needs_preview(mine: &Holdings, theirs: &Holdings) -> bool {
    !mine.is_empty() && !theirs.is_empty()
}

/// The Lists that will become one: those named alike on both devices, by the rule the List name
/// clash merge uses (`todo::name_key`: case and accents ignored). Each is given by this
/// device's spelling, once, in alphabetical order.
pub fn lists_that_combine(mine: &Holdings, theirs: &Holdings) -> Vec<String> {
    let theirs: std::collections::HashSet<String> =
        theirs.lists.iter().map(|n| fold(n.trim())).collect();
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<String> = mine
        .lists
        .iter()
        .filter(|n| {
            let key = fold(n.trim());
            theirs.contains(&key) && seen.insert(key)
        })
        .cloned()
        .collect();
    out.sort_by_key(|n| fold(n.trim()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held(entries: u32, tasks: u32, lists: &[&str]) -> Holdings {
        Holdings {
            entries,
            tasks,
            lists: lists.iter().map(|n| (*n).to_owned()).collect(),
        }
    }

    #[test]
    fn counts_are_taken_as_they_are() {
        let c = Counts::of(&held(12, 30, &["A", "B", "C", "D"]));
        assert_eq!((c.entries, c.tasks, c.lists), (12, 30, 4));
    }

    #[test]
    fn only_two_devices_with_data_need_the_preview() {
        let some = held(0, 1, &[]);
        assert!(needs_preview(&some, &some));
        assert!(!needs_preview(&Holdings::default(), &some));
        assert!(!needs_preview(&some, &Holdings::default()));
        assert!(!needs_preview(&Holdings::default(), &Holdings::default()));
        // A device with only a List is not empty.
        assert!(needs_preview(&held(0, 0, &["Home"]), &some));
    }

    #[test]
    fn lists_combine_when_named_alike_ignoring_case_and_accents() {
        let here = held(1, 1, &["Work", "Café", "Home", "groceries"]);
        let there = held(1, 1, &["GROCERIES", "cafe", "Errands", "Work "]);
        assert_eq!(
            lists_that_combine(&here, &there),
            ["Café", "groceries", "Work"]
        );
        assert!(lists_that_combine(&here, &held(1, 1, &["Other"])).is_empty());
    }

    #[test]
    fn a_name_used_twice_here_is_listed_once() {
        let here = held(1, 1, &["Work", "work"]);
        assert_eq!(lists_that_combine(&here, &held(1, 1, &["WORK"])), ["Work"]);
    }
}
