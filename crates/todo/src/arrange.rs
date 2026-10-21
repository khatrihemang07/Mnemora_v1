//! Arranging: how the open Tasks of a view are grouped and sorted, and the Preferences that choose
//! it. Group first, then sort inside each group. Pure functions over Tasks (the views fetch and
//! hand them in), so every combination is cheap to test. Task text never reaches a log line.

use std::cmp::Ordering;

use mnemora_core::settings;
use mnemora_core::text::fold;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

use mnemora_core::error::AppResult;

use crate::MODULE_ID;
use crate::list::{ListEntry, TodoList};
use crate::task::Task;
use crate::when::add_days;

/// What a view groups its Tasks by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    /// One plain group.
    #[default]
    None,
    /// One group per Priority.
    Priority,
    /// One group per List, the Inbox first.
    List,
    /// One group per Due date (Upcoming).
    Day,
    /// Overdue, Today, Tomorrow, the next days, later, and no date.
    Due,
}

impl GroupBy {
    /// The stored text.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Priority => "priority",
            Self::List => "list",
            Self::Day => "day",
            Self::Due => "due",
        }
    }

    /// Reads stored text.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "none" => Self::None,
            "priority" => Self::Priority,
            "list" => Self::List,
            "day" => Self::Day,
            "due" => Self::Due,
            _ => return None,
        })
    }
}

/// How the Tasks inside one group are ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TaskSort {
    /// The drag order.
    #[default]
    Manual,
    /// Earliest Due date and time first; undated last.
    Due,
    /// Most urgent Priority first.
    Priority,
    /// Title, A to Z, ignoring case and accents.
    Title,
    /// Newest first.
    Added,
}

impl TaskSort {
    /// The stored text.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Due => "due",
            Self::Priority => "priority",
            Self::Title => "title",
            Self::Added => "added",
        }
    }

    /// Reads stored text.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "manual" => Self::Manual,
            "due" => Self::Due,
            "priority" => Self::Priority,
            "title" => Self::Title,
            "added" => Self::Added,
            _ => return None,
        })
    }
}

/// How the Lists in the sidebar are ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ListSort {
    /// The drag order.
    #[default]
    Manual,
    /// Name, A to Z.
    NameAsc,
    /// Name, Z to A.
    NameDesc,
    /// Most open Tasks first.
    MostOpen,
    /// Oldest first.
    Created,
}

impl ListSort {
    /// Reads stored text.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "manual" => Self::Manual,
            "name_asc" => Self::NameAsc,
            "name_desc" => Self::NameDesc,
            "most_open" => Self::MostOpen,
            "created" => Self::Created,
            _ => return None,
        })
    }
}

/// What a List page does with its Done Tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DoneInLists {
    /// Done today struck through at the bottom, older ones behind "Done earlier".
    #[default]
    TodayShown,
    /// All behind one fold.
    Folded,
    /// All shown, struck through.
    Shown,
    /// None.
    Hidden,
}

impl DoneInLists {
    /// Reads stored text.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "today_shown" => Self::TodayShown,
            "folded" => Self::Folded,
            "shown" => Self::Shown,
            "hidden" => Self::Hidden,
            _ => return None,
        })
    }
}

/// A slice of time by Due date, for grouping by Due date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DueBucket {
    /// Before today.
    Overdue,
    /// Today.
    Today,
    /// Tomorrow.
    Tomorrow,
    /// The six days after tomorrow.
    Week,
    /// Further away.
    Later,
    /// No Due date.
    NoDate,
}

impl DueBucket {
    const ALL: [Self; 6] = [
        Self::Overdue,
        Self::Today,
        Self::Tomorrow,
        Self::Week,
        Self::Later,
        Self::NoDate,
    ];
}

/// Which group of a view a Task is in: what a heading says and what a drop adopts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GroupKey {
    /// The one group of an ungrouped view.
    Plain,
    /// Due before today, on top in Today and Upcoming.
    Overdue,
    /// One Priority, 1 to 4 (4 is "No priority").
    Priority {
        /// 1 to 4.
        priority: u32,
    },
    /// One List; none is the Inbox.
    List {
        /// The List.
        list_id: Option<String>,
    },
    /// One day.
    Day {
        /// `YYYY-MM-DD`.
        date: String,
    },
    /// A slice of time by Due date.
    Due {
        /// Which.
        bucket: DueBucket,
    },
    /// The Inbox's "No date, in a List".
    NoDateInList,
}

/// Open Tasks of one group, with what a Task dropped (or added) there takes on.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
pub struct TaskGroup {
    /// Which group.
    pub key: GroupKey,
    /// The properties a Task dropped here adopts, in `todo_task_reorder`'s terms: a day sets the
    /// date, a Priority sets the Priority, a List moves it. Overdue adopts nothing: dragging
    /// among Overdue Tasks only reorders.
    pub adopt: Vec<GroupKey>,
    /// Its Tasks, sorted.
    pub tasks: Vec<Task>,
}

/// Compares two Tasks the way a sort says. Ties fall back to the manual order.
pub fn compare(sort: TaskSort) -> impl Fn(&Task, &Task) -> Ordering {
    move |a, b| {
        let primary = match sort {
            TaskSort::Manual => Ordering::Equal,
            TaskSort::Due => undated_last(a.due_date.as_deref(), b.due_date.as_deref())
                .then_with(|| undated_last(a.due_time.as_deref(), b.due_time.as_deref())),
            TaskSort::Priority => a.priority.cmp(&b.priority),
            TaskSort::Title => fold(&a.title).cmp(&fold(&b.title)),
            // Tasks made in the same millisecond keep the order they were added in.
            TaskSort::Added => b
                .created_at
                .cmp(&a.created_at)
                .then_with(|| b.sort_key.cmp(&a.sort_key)),
        };
        primary
            .then_with(|| a.sort_key.cmp(&b.sort_key))
            .then_with(|| a.id.cmp(&b.id))
    }
}

/// Earlier values first, none last.
fn undated_last(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (a, b) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Where a Task's Due date falls, given today.
pub fn due_bucket(due: Option<&str>, today: &str) -> DueBucket {
    let Some(due) = due else {
        return DueBucket::NoDate;
    };
    if due < today {
        return DueBucket::Overdue;
    }
    if due == today {
        return DueBucket::Today;
    }
    let after = |days| add_days(today, days);
    if after(1).as_deref() == Some(due) {
        DueBucket::Tomorrow
    } else if after(7).is_some_and(|end| due <= end.as_str()) {
        DueBucket::Week
    } else {
        DueBucket::Later
    }
}

/// What groups need to know besides the Tasks.
pub struct GroupContext<'a> {
    /// Lists in the order the sidebar shows them.
    pub lists: &'a [TodoList],
    /// Today, `YYYY-MM-DD`.
    pub today: &'a str,
}

/// Removes the Tasks `keep` accepts from `tasks` and returns them, in order.
fn take(tasks: &mut Vec<Task>, keep: &dyn Fn(&Task) -> bool) -> Vec<Task> {
    let (taken, rest): (Vec<_>, Vec<_>) = tasks.drain(..).partition(|t| keep(t));
    *tasks = rest;
    taken
}

/// Groups Tasks and sorts each group with `cmp`. Empty groups are left out, except the plain
/// group of an ungrouped view, which is always there (it is where "+ New task" lives).
pub fn group_tasks(
    mut tasks: Vec<Task>,
    by: GroupBy,
    ctx: &GroupContext<'_>,
    cmp: &dyn Fn(&Task, &Task) -> Ordering,
) -> Vec<TaskGroup> {
    tasks.sort_by(cmp);
    let mut groups = Vec::new();
    let mut push = |key: GroupKey, adopt: Vec<GroupKey>, tasks: Vec<Task>| {
        if !tasks.is_empty() {
            groups.push(TaskGroup { key, adopt, tasks });
        }
    };
    match by {
        GroupBy::None => {
            return vec![TaskGroup {
                key: GroupKey::Plain,
                adopt: Vec::new(),
                tasks,
            }];
        }
        GroupBy::Priority => {
            for p in 1..=4 {
                let key = GroupKey::Priority { priority: p };
                push(
                    key.clone(),
                    vec![key],
                    take(&mut tasks, &|t| t.priority == p),
                );
            }
        }
        GroupBy::List => {
            let ids = std::iter::once(None).chain(ctx.lists.iter().map(|l| Some(l.id.clone())));
            for list_id in ids {
                let key = GroupKey::List {
                    list_id: list_id.clone(),
                };
                push(
                    key.clone(),
                    vec![key],
                    take(&mut tasks, &|t| t.list_id == list_id),
                );
            }
        }
        GroupBy::Day => {
            let mut dates: Vec<String> = tasks.iter().filter_map(|t| t.due_date.clone()).collect();
            dates.sort();
            dates.dedup();
            for date in dates {
                let key = GroupKey::Day { date: date.clone() };
                push(
                    key.clone(),
                    vec![key],
                    take(&mut tasks, &|t| {
                        t.due_date.as_deref() == Some(date.as_str())
                    }),
                );
            }
            push(GroupKey::Plain, Vec::new(), take(&mut tasks, &|_| true));
        }
        GroupBy::Due => {
            for bucket in DueBucket::ALL {
                let adopt = match bucket {
                    DueBucket::Today => vec![GroupKey::Day {
                        date: ctx.today.to_owned(),
                    }],
                    DueBucket::Tomorrow => add_days(ctx.today, 1)
                        .map(|date| GroupKey::Day { date })
                        .into_iter()
                        .collect(),
                    _ => Vec::new(),
                };
                push(
                    GroupKey::Due { bucket },
                    adopt,
                    take(&mut tasks, &|t| {
                        due_bucket(t.due_date.as_deref(), ctx.today) == bucket
                    }),
                );
            }
        }
    }
    groups
}

/// Sorts Lists the way the sidebar shows them. Ties (and Manual) keep the drag order, which
/// `lists` arrives in.
pub fn sort_lists(entries: &mut [ListEntry], sort: ListSort) {
    match sort {
        ListSort::Manual => {}
        ListSort::NameAsc => entries.sort_by_cached_key(|e| fold(&e.list.name)),
        ListSort::NameDesc => {
            entries.sort_by_cached_key(|e| std::cmp::Reverse(fold(&e.list.name)));
        }
        ListSort::MostOpen => entries.sort_by_key(|e| std::cmp::Reverse(e.open_count)),
        ListSort::Created => entries.sort_by_key(|e| {
            (
                e.list.created_at,
                e.list.sort_key.clone(),
                e.list.id.clone(),
            )
        }),
    }
}

/// Today's Preferences for the views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewPrefs {
    /// Today's grouping: None, Priority or List.
    pub group_today: GroupBy,
    /// Upcoming's grouping: Day, Priority or List.
    pub group_upcoming: GroupBy,
    /// The Inbox's grouping: None, Priority or Due date.
    pub group_inbox: GroupBy,
    /// The All view's grouping: List, Priority, Due date or None.
    pub group_all: GroupBy,
    /// How the Inbox sorts inside its groups.
    pub inbox_sort: TaskSort,
    /// How the Lists sort in the sidebar.
    pub lists_sort: ListSort,
    /// Done Tasks on a List page.
    pub done_in_lists: DoneInLists,
}

/// The Preferences' keys, with the values each accepts: the first is the default.
pub const GROUP_TODAY: (&str, &[&str]) = ("group_today", &["priority", "none", "list"]);
/// Upcoming's grouping.
pub const GROUP_UPCOMING: (&str, &[&str]) = ("group_upcoming", &["day", "priority", "list"]);
/// The Inbox's grouping.
pub const GROUP_INBOX: (&str, &[&str]) = ("group_inbox", &["none", "priority", "due"]);
/// The All view's grouping.
pub const GROUP_ALL: (&str, &[&str]) = ("group_all", &["list", "priority", "due", "none"]);
/// The Inbox's sort.
pub const INBOX_SORT: (&str, &[&str]) = (
    "inbox_sort",
    &["manual", "due", "priority", "title", "added"],
);
/// The Lists' sort.
pub const LISTS_SORT: (&str, &[&str]) = (
    "lists_sort",
    &["manual", "name_asc", "name_desc", "most_open", "created"],
);
/// Done Tasks in Lists.
pub const DONE_IN_LISTS: (&str, &[&str]) = (
    "done_in_lists",
    &["today_shown", "folded", "shown", "hidden"],
);

/// One Preference: the saved text if it is one of the allowed values, else the default.
fn pref<T>(
    conn: &Connection,
    (key, allowed): (&str, &[&str]),
    parse: fn(&str) -> Option<T>,
) -> AppResult<T> {
    let saved = settings::stored(conn, MODULE_ID, key)?;
    let text = match &saved {
        Some(Value::String(s)) if allowed.contains(&s.as_str()) => s.as_str(),
        _ => allowed[0],
    };
    // The registry only ever lists values that parse.
    Ok(parse(text).unwrap_or_else(|| unreachable!("{key} lists an unparsable default")))
}

impl ViewPrefs {
    /// Reads every Preference; a missing or invalid one is its default.
    pub fn read(conn: &Connection) -> AppResult<Self> {
        Ok(Self {
            group_today: pref(conn, GROUP_TODAY, GroupBy::parse)?,
            group_upcoming: pref(conn, GROUP_UPCOMING, GroupBy::parse)?,
            group_inbox: pref(conn, GROUP_INBOX, GroupBy::parse)?,
            group_all: pref(conn, GROUP_ALL, GroupBy::parse)?,
            inbox_sort: pref(conn, INBOX_SORT, TaskSort::parse)?,
            lists_sort: pref(conn, LISTS_SORT, ListSort::parse)?,
            done_in_lists: pref(conn, DONE_IN_LISTS, DoneInLists::parse)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::TaskState;

    fn task(title: &str, key: &str, priority: u32, due: Option<&str>, list: Option<&str>) -> Task {
        Task {
            id: format!("id-{title}"),
            list_id: list.map(str::to_owned),
            title: title.to_owned(),
            notes: String::new(),
            due_date: due.map(str::to_owned),
            due_time: None,
            repeat_freq: None,
            repeat_every: None,
            repeat_days: None,
            repeat_mode: None,
            priority,
            state: TaskState::Open,
            closed_at: None,
            repeat_of: None,
            understood: None,
            sort_key: key.to_owned(),
            created_at: 0,
            updated_at: 0,
            comment_count: 0,
        }
    }

    fn titles(groups: &[TaskGroup]) -> Vec<Vec<&str>> {
        groups
            .iter()
            .map(|g| g.tasks.iter().map(|t| t.title.as_str()).collect())
            .collect()
    }

    const TODAY: &str = "2026-10-21";

    #[test]
    fn due_dates_fall_into_buckets() {
        let at = |d| due_bucket(d, TODAY);
        assert_eq!(at(None), DueBucket::NoDate);
        assert_eq!(at(Some("2026-10-20")), DueBucket::Overdue);
        assert_eq!(at(Some("2026-10-21")), DueBucket::Today);
        assert_eq!(at(Some("2026-10-22")), DueBucket::Tomorrow);
        assert_eq!(at(Some("2026-10-23")), DueBucket::Week);
        assert_eq!(at(Some("2026-10-28")), DueBucket::Week);
        assert_eq!(at(Some("2026-10-29")), DueBucket::Later);
    }

    #[test]
    fn sorts_order_tasks_and_ties_keep_the_drag_order() {
        let mut a = task("b", "3", 2, Some("2026-10-25"), None);
        a.created_at = 10;
        let mut b = task("A", "1", 4, None, None);
        b.created_at = 30;
        let mut c = task("c", "2", 2, Some("2026-10-22"), None);
        c.created_at = 20;
        let order = |sort| {
            let mut v = [a.clone(), b.clone(), c.clone()];
            v.sort_by(compare(sort));
            v.iter().map(|t| t.title.clone()).collect::<Vec<_>>()
        };
        assert_eq!(order(TaskSort::Manual), ["A", "c", "b"]);
        assert_eq!(order(TaskSort::Due), ["c", "b", "A"]);
        assert_eq!(order(TaskSort::Priority), ["c", "b", "A"]);
        assert_eq!(order(TaskSort::Title), ["A", "b", "c"]);
        assert_eq!(order(TaskSort::Added), ["A", "c", "b"]);
    }

    #[test]
    fn grouping_buckets_then_sorts_inside_each_group() {
        let lists = [TodoList {
            id: "L1".into(),
            name: "Home".into(),
            emoji: None,
            sort_key: "V".into(),
            task_sort: TaskSort::Manual,
            group_by: GroupBy::None,
            created_at: 0,
            updated_at: 0,
        }];
        let ctx = GroupContext {
            lists: &lists,
            today: TODAY,
        };
        let tasks = vec![
            task("a", "1", 3, None, None),
            task("b", "2", 1, Some("2026-10-21"), Some("L1")),
            task("c", "3", 3, Some("2026-10-30"), Some("L1")),
            task("d", "4", 1, Some("2026-10-20"), None),
        ];
        let cmp = compare(TaskSort::Manual);
        let by = |by| group_tasks(tasks.clone(), by, &ctx, &cmp);
        assert_eq!(titles(&by(GroupBy::None)), [["a", "b", "c", "d"]]);
        let p = by(GroupBy::Priority);
        assert_eq!(titles(&p), [vec!["b", "d"], vec!["a", "c"]]);
        assert_eq!(p[0].key, GroupKey::Priority { priority: 1 });
        let l = by(GroupBy::List);
        assert_eq!(titles(&l), [vec!["a", "d"], vec!["b", "c"]]);
        assert_eq!(l[0].key, GroupKey::List { list_id: None });
        let d = by(GroupBy::Due);
        assert_eq!(titles(&d), [vec!["d"], vec!["b"], vec!["c"], vec!["a"]]);
        assert_eq!(d[1].adopt, [GroupKey::Day { date: TODAY.into() }]);
        assert!(d[0].adopt.is_empty());
        let days = by(GroupBy::Day);
        assert_eq!(titles(&days), [vec!["d"], vec!["b"], vec!["c"], vec!["a"]]);
        assert_eq!(days[3].key, GroupKey::Plain);
    }

    #[test]
    fn lists_sort_by_name_open_count_and_creation() {
        let entry = |name: &str, open, created| ListEntry {
            list: TodoList {
                id: name.to_owned(),
                name: name.to_owned(),
                emoji: None,
                sort_key: String::new(),
                task_sort: TaskSort::Manual,
                group_by: GroupBy::None,
                created_at: created,
                updated_at: 0,
            },
            open_count: open,
            task_count: open,
        };
        let base = vec![
            entry("beta", 2, 3),
            entry("Álpha", 5, 2),
            entry("gamma", 2, 1),
        ];
        let names = |sort| {
            let mut v = base.clone();
            sort_lists(&mut v, sort);
            v.iter().map(|e| e.list.name.clone()).collect::<Vec<_>>()
        };
        assert_eq!(names(ListSort::Manual), ["beta", "Álpha", "gamma"]);
        assert_eq!(names(ListSort::NameAsc), ["Álpha", "beta", "gamma"]);
        assert_eq!(names(ListSort::NameDesc), ["gamma", "beta", "Álpha"]);
        assert_eq!(names(ListSort::MostOpen), ["Álpha", "beta", "gamma"]);
        assert_eq!(names(ListSort::Created), ["gamma", "Álpha", "beta"]);
    }
}
