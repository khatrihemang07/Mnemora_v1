//! What the Todo widget shows: a view turned into display-ready rows and text. Every word in a
//! snapshot is final, so the Android side does no date or wording logic. The groups, order and
//! done-hiding are the app's own (`today`); this module only picks and words them.

use std::collections::HashMap;

use mnemora_core::error::AppResult;
use mnemora_core::settings;
use rusqlite::Connection;
use serde::Serialize;

use crate::arrange::{
    DueBucket, GroupBy, GroupContext, GroupKey, TaskGroup, TaskSort, ViewPrefs, compare,
    group_tasks,
};
use crate::error::TodoError;
use crate::list::TodoList;
use crate::list_repo;
use crate::list_service::ordered_lists;
use crate::repo;
use crate::schedule::{counts, today, week_ahead};
use crate::service::inbox;
use crate::task::Task;
use crate::when::LocalNow;
use crate::widget_text::{
    TimeFormat, day_heading, format_hm, overdue_day_label, plain_title, row_date_label,
    row_day_label,
};

/// How many days ahead the Upcoming view looks.
const UPCOMING_DAYS: i64 = 7;

/// Which Todo view a widget shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetView {
    /// Overdue, then what is due today.
    Today,
    /// Overdue, then each day of the next 7.
    Upcoming,
    /// Tasks with no List, then the undated Tasks that sit in a List.
    Inbox,
    /// Every open Task, by List with the Inbox first.
    All,
    /// One List, by id.
    List(String),
}

impl WidgetView {
    /// Reads the view's name as the widget stores it: `today`, `upcoming`, `inbox`, `all` or
    /// `list:<id>`.
    pub fn parse(name: &str) -> Result<Self, TodoError> {
        match name {
            "today" => Ok(Self::Today),
            "upcoming" => Ok(Self::Upcoming),
            "inbox" => Ok(Self::Inbox),
            "all" => Ok(Self::All),
            _ => match name.strip_prefix("list:") {
                Some(id) if !id.is_empty() => Ok(Self::List(id.to_owned())),
                _ => Err(TodoError::Validation("That view is not available")),
            },
        }
    }

    /// The name `parse` reads.
    fn name(&self) -> String {
        match self {
            Self::Today => "today".to_owned(),
            Self::Upcoming => "upcoming".to_owned(),
            Self::Inbox => "inbox".to_owned(),
            Self::All => "all".to_owned(),
            Self::List(id) => format!("list:{id}"),
        }
    }
}

/// How a piece of text should be coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    /// The theme's usual colour.
    Normal,
    /// Red: something is late.
    Overdue,
}

/// The header's count: `3 left`, or a red `2 overdue`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HeaderCount {
    /// The words.
    pub text: String,
    /// Red when something is Overdue.
    pub tone: Tone,
}

/// One Task row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WidgetRow {
    /// The Task's id: what a tick or a tap acts on.
    pub id: String,
    /// The title without Markdown marks.
    pub title: String,
    /// The diamond's ring: `p1` red, `p2` amber, `p3` violet, `none`.
    pub ring: &'static str,
    /// Whether the Task is Overdue (the diamond's edge turns reddish).
    pub overdue: bool,
    /// The time on the right, always grey.
    pub time: Option<String>,
    /// For an Overdue row, the day it was due, in red: `Mon`, `Yesterday`.
    pub day: Option<String>,
    /// The List chip in a mixed view: emoji and name. None in the Inbox and in a List's own view.
    pub list: Option<String>,
}

/// A run of rows under one heading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WidgetGroup {
    /// `Overdue`, `Today`, a day...
    pub heading: Option<String>,
    /// Red for Overdue.
    pub tone: Tone,
    /// Its Tasks, in the app's order.
    pub rows: Vec<WidgetRow>,
}

/// What shows when there is nothing to list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmptyState {
    /// The big line.
    pub title: String,
    /// The helpful line under it.
    pub line: String,
}

/// A view as the widget draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WidgetSnapshot {
    /// The view's name as asked for (`today`, `list:<id>`).
    pub view: String,
    /// Settings ▸ Theme (`system`, `light`, `dark`, `oled`): what "Same as app" follows.
    pub app_theme: String,
    /// The header's name for it.
    pub title: String,
    /// The header's count, if it has one.
    pub count: Option<HeaderCount>,
    /// Rows by heading; empty when `empty` is set.
    pub groups: Vec<WidgetGroup>,
    /// Set when there is nothing to list.
    pub empty: Option<EmptyState>,
}

/// The rows' facts shared by every row of one snapshot.
struct Context<'a> {
    local: &'a LocalNow,
    format: TimeFormat,
    lists: HashMap<String, String>,
}

impl Context<'_> {
    fn row(&self, task: &Task, show_list: bool, show_day: bool) -> WidgetRow {
        // Late wherever it shows: the diamond's edge turns reddish and the day it was due is red.
        let overdue = task
            .due_date
            .as_deref()
            .is_some_and(|d| d < self.local.today.as_str());
        WidgetRow {
            id: task.id.clone(),
            title: plain_title(&task.title),
            ring: match task.priority {
                1 => "p1",
                2 => "p2",
                3 => "p3",
                _ => "none",
            },
            overdue,
            // An Overdue row shows the day it was due where the time would go.
            time: self.when(task, overdue, show_day),
            day: task
                .due_date
                .as_deref()
                .filter(|_| overdue)
                .map(|d| overdue_day_label(d, &self.local.today)),
            list: task
                .list_id
                .as_ref()
                .filter(|_| show_list)
                .and_then(|id| self.lists.get(id))
                .cloned(),
        }
    }
}

impl Context<'_> {
    /// The grey text on the right: the time, with the day in front when the Task is due on a day
    /// other than today and the view's heading does not already say it (`Tomorrow 11:00`,
    /// `Wed 4 Nov`). Overdue rows show their red day instead.
    fn when(&self, task: &Task, overdue: bool, show_day: bool) -> Option<String> {
        if overdue {
            return None;
        }
        let time = task.due_time.as_deref().map(|t| format_hm(t, self.format));
        let day = task
            .due_date
            .as_deref()
            .filter(|d| show_day && *d != self.local.today)
            .map(|d| row_day_label(d, &self.local.today));
        match (day, time) {
            (Some(d), Some(t)) => Some(format!("{d} {t}")),
            (day, time) => day.or(time),
        }
    }
}

/// The List chip's text: emoji and name.
fn chip(emoji: Option<&str>, name: &str) -> String {
    match emoji {
        Some(e) if !e.is_empty() => format!("{e} {name}"),
        _ => name.to_owned(),
    }
}

pub(crate) fn list_chip(list: &TodoList) -> String {
    chip(list.emoji.as_deref(), &list.name)
}

/// A List the setup screen can offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WidgetList {
    /// The List's id: what `list:<id>` names.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its emoji, if it has one.
    pub emoji: Option<String>,
    /// Emoji and name together, as a chip or a title.
    pub label: String,
}

/// What the setup screen needs: the Lists to pick from, and the app's theme to dress itself in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WidgetLists {
    /// Settings ▸ Theme (`system`, `light`, `dark`, `oled`).
    pub app_theme: String,
    /// The Lists in the sidebar's order.
    pub lists: Vec<WidgetList>,
}

fn app_theme(conn: &Connection) -> AppResult<String> {
    let stored = settings::stored(conn, "core", "theme")?;
    Ok(match stored.as_ref().and_then(|v| v.as_str()) {
        Some(t @ ("system" | "light" | "dark" | "oled")) => t.to_owned(),
        _ => "system".to_owned(),
    })
}

/// The Lists for the setup screen, in the order the sidebar shows them.
pub fn lists(conn: &Connection) -> AppResult<WidgetLists> {
    let prefs = ViewPrefs::read(conn)?;
    let lists = ordered_lists(conn, prefs.lists_sort)?
        .into_iter()
        .map(|e| WidgetList {
            label: list_chip(&e.list),
            id: e.list.id,
            name: e.list.name,
            emoji: e.list.emoji,
        })
        .collect();
    Ok(WidgetLists {
        app_theme: app_theme(conn)?,
        lists,
    })
}

/// Whether saving `keys` of Module `module` changes what the Todo widget draws: the app's theme,
/// the time format and week start, and how Todo views group and sort.
pub fn settings_affect_widget(module: &str, keys: &[String]) -> bool {
    let draws = |key: &str| match module {
        "core" => matches!(key, "theme" | "time_format" | "week_start"),
        crate::MODULE_ID => crate::arrange::PREF_KEYS.contains(&key),
        _ => false,
    };
    keys.iter().any(|k| draws(k))
}

/// Builds `view` for the widget as of `local`. A List that no longer exists is `NotFound`.
pub fn snapshot(
    conn: &Connection,
    view: &WidgetView,
    local: &LocalNow,
) -> AppResult<WidgetSnapshot> {
    local.check()?;
    let format = TimeFormat::read(conn)?;
    let lists = list_repo::all(conn)?
        .iter()
        .map(|l| (l.id.clone(), list_chip(l)))
        .collect();
    let cx = Context {
        local,
        format,
        lists,
    };
    let mut snap = match view {
        WidgetView::Today => today_snapshot(conn, &cx)?,
        WidgetView::Upcoming => upcoming_snapshot(conn, &cx)?,
        WidgetView::Inbox => inbox_snapshot(conn, &cx)?,
        WidgetView::All => all_snapshot(conn, &cx)?,
        WidgetView::List(id) => list_snapshot(conn, &cx, id)?,
    };
    snap.view = view.name();
    snap.app_theme = app_theme(conn)?;
    Ok(snap)
}

/// The Overdue run of one snapshot: red heading, then its rows.
fn overdue_group(rows: Vec<WidgetRow>) -> Option<WidgetGroup> {
    (!rows.is_empty()).then(|| WidgetGroup {
        heading: Some("Overdue".to_owned()),
        tone: Tone::Overdue,
        rows,
    })
}

fn today_snapshot(conn: &Connection, cx: &Context<'_>) -> AppResult<WidgetSnapshot> {
    let app = today(conn, cx.local)?;
    let (mut late, mut due) = (Vec::new(), Vec::new());
    for group in &app.groups {
        let rows = group.tasks.iter().map(|t| cx.row(t, true, false));
        if group.key == GroupKey::Overdue {
            late.extend(rows);
        } else {
            // The app may split Today by Priority or List; the widget shows one run, in that order.
            due.extend(rows);
        }
    }
    let count = if !late.is_empty() {
        Some(HeaderCount {
            text: format!("{} overdue", late.len()),
            tone: Tone::Overdue,
        })
    } else if !due.is_empty() {
        Some(HeaderCount {
            text: format!("{} left", due.len()),
            tone: Tone::Normal,
        })
    } else {
        None
    };
    let mut groups = Vec::new();
    groups.extend(overdue_group(late));
    if !due.is_empty() {
        groups.push(WidgetGroup {
            heading: Some("Today".to_owned()),
            tone: Tone::Normal,
            rows: due,
        });
    }
    let empty = if groups.is_empty() {
        let line = match counts(conn, cx.local)?.next {
            Some(next) => next_line(&next, cx),
            None => "Nothing coming up".to_owned(),
        };
        Some(EmptyState {
            title: "Nothing due today".to_owned(),
            line,
        })
    } else {
        None
    };
    Ok(finish("Today".to_owned(), count, groups, empty))
}

/// Overdue, then each day of the next seven. Always by day, in the app's date order.
fn upcoming_snapshot(conn: &Connection, cx: &Context<'_>) -> AppResult<WidgetSnapshot> {
    let (late, days) = week_ahead(conn, cx.local, UPCOMING_DAYS)?;
    let mut groups = Vec::new();
    groups.extend(overdue_group(
        late.iter().map(|t| cx.row(t, true, false)).collect(),
    ));
    for day in &days {
        let GroupKey::Day { date } = &day.key else {
            continue;
        };
        groups.push(WidgetGroup {
            heading: Some(day_heading(date, &cx.local.today)),
            tone: Tone::Normal,
            rows: day.tasks.iter().map(|t| cx.row(t, true, false)).collect(),
        });
    }
    let empty = EmptyState {
        title: "Nothing coming up".to_owned(),
        line: "Nothing in the next 7 days".to_owned(),
    };
    Ok(counted("Upcoming".to_owned(), groups, empty))
}

/// The Inbox as the app groups and sorts it, then its "No date, in a List" group.
fn inbox_snapshot(conn: &Connection, cx: &Context<'_>) -> AppResult<WidgetSnapshot> {
    let app = inbox(conn, cx.local)?;
    let groups = app
        .groups
        .iter()
        .map(|g| {
            // Tasks in the Inbox have no List to name; the undated ones in a List do.
            let show_list = g.key == GroupKey::NoDateInList;
            cx.group(g, show_list)
        })
        .collect::<Vec<_>>();
    let empty = EmptyState {
        title: "Inbox zero".to_owned(),
        line: "Everything has a List".to_owned(),
    };
    Ok(counted("Inbox".to_owned(), groups, empty))
}

/// Every open Task by List, the Inbox first, in the sidebar's order of Lists.
fn all_snapshot(conn: &Connection, cx: &Context<'_>) -> AppResult<WidgetSnapshot> {
    let prefs = ViewPrefs::read(conn)?;
    let lists: Vec<TodoList> = ordered_lists(conn, prefs.lists_sort)?
        .into_iter()
        .map(|e| e.list)
        .collect();
    let ctx = GroupContext {
        lists: &lists,
        today: &cx.local.today,
    };
    let mut tasks = repo::inbox(conn)?;
    tasks.extend(repo::open_in_lists(conn)?);
    let groups = group_tasks(tasks, GroupBy::List, &ctx, &compare(TaskSort::Manual))
        .iter()
        .map(|g| cx.group(g, false))
        .collect::<Vec<_>>();
    let empty = EmptyState {
        title: "No open Tasks".to_owned(),
        line: "Everything is done".to_owned(),
    };
    Ok(counted("All".to_owned(), groups, empty))
}

/// One List, grouped and sorted as its own page does.
fn list_snapshot(conn: &Connection, cx: &Context<'_>, id: &str) -> AppResult<WidgetSnapshot> {
    let list = list_repo::get(conn, id)?.ok_or(TodoError::ListNotFound)?;
    let ctx = GroupContext {
        lists: &[],
        today: &cx.local.today,
    };
    let groups = group_tasks(
        repo::open_in_list(conn, id)?,
        list.group_by,
        &ctx,
        &compare(list.task_sort),
    )
    .iter()
    .map(|g| cx.group(g, false))
    .collect::<Vec<_>>();
    let title = list_chip(&list);
    let empty = EmptyState {
        title: "No open Tasks".to_owned(),
        line: format!("in {title}"),
    };
    Ok(counted(title, groups, empty))
}

impl Context<'_> {
    /// One of the app's groups as the widget shows it. Empty groups are dropped by the callers
    /// through `counted`.
    fn group(&self, group: &TaskGroup, show_list: bool) -> WidgetGroup {
        let (heading, tone) = self.heading(&group.key);
        WidgetGroup {
            heading,
            tone,
            rows: group
                .tasks
                .iter()
                .map(|t| self.row(t, show_list, true))
                .collect(),
        }
    }

    /// What heads a group, in the app's words.
    fn heading(&self, key: &GroupKey) -> (Option<String>, Tone) {
        let plain = |text: &str| (Some(text.to_owned()), Tone::Normal);
        match key {
            GroupKey::Plain => (None, Tone::Normal),
            GroupKey::Overdue => (Some("Overdue".to_owned()), Tone::Overdue),
            GroupKey::Priority { priority: 4 } => plain("No priority"),
            GroupKey::Priority { priority } => (Some(format!("P{priority}")), Tone::Normal),
            GroupKey::List { list_id: None } => plain("Inbox"),
            GroupKey::List { list_id: Some(id) } => (self.lists.get(id).cloned(), Tone::Normal),
            GroupKey::Day { date } if *date == self.local.today => plain("Today"),
            GroupKey::Day { date } => (Some(day_heading(date, &self.local.today)), Tone::Normal),
            GroupKey::Due { bucket } => match bucket {
                DueBucket::Overdue => (Some("Overdue".to_owned()), Tone::Overdue),
                DueBucket::Today => plain("Today"),
                DueBucket::Tomorrow => plain("Tomorrow"),
                DueBucket::Week => plain("Next 7 days"),
                DueBucket::Later => plain("Later"),
                DueBucket::NoDate => plain("No date"),
            },
            GroupKey::NoDateInList => plain("No date, in a List"),
        }
    }
}

/// A snapshot whose header counts the rows shown (views other than Today): groups without Tasks
/// are dropped.
fn counted(title: String, mut groups: Vec<WidgetGroup>, empty: EmptyState) -> WidgetSnapshot {
    groups.retain(|g| !g.rows.is_empty());
    let n: usize = groups.iter().map(|g| g.rows.len()).sum();
    let count = (n > 0).then(|| HeaderCount {
        text: n.to_string(),
        tone: Tone::Normal,
    });
    let empty = groups.is_empty().then_some(empty);
    finish(title, count, groups, empty)
}

fn finish(
    title: String,
    count: Option<HeaderCount>,
    groups: Vec<WidgetGroup>,
    empty: Option<EmptyState>,
) -> WidgetSnapshot {
    WidgetSnapshot {
        // `snapshot` names the view and the app's theme.
        view: String::new(),
        app_theme: String::new(),
        title,
        count,
        groups,
        empty,
    }
}

/// `Next: Dentist, tomorrow 11:00`.
fn next_line(next: &Task, cx: &Context<'_>) -> String {
    let day = next
        .due_date
        .as_deref()
        .map(|d| row_date_label(d, &cx.local.today))
        // "Tomorrow" reads as part of a sentence in lower case; weekdays and dates do not.
        .map(|d| if d == "Tomorrow" { "tomorrow".to_owned() } else { d });
    let time = next.due_time.as_deref().map(|t| format_hm(t, cx.format));
    let when = [day, time]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    format!("Next: {}, {when}", plain_title(&next.title))
}
