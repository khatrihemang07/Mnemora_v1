//! What the Todo widget shows: a view turned into display-ready rows and text. Every word in a
//! snapshot is final, so the Android side does no date or wording logic. The groups, order and
//! done-hiding are the app's own (`today`); this module only picks and words them.

use std::collections::HashMap;

use mnemora_core::error::AppResult;
use mnemora_core::settings;
use rusqlite::Connection;
use serde::Serialize;

use crate::arrange::GroupKey;
use crate::error::TodoError;
use crate::list_repo;
use crate::schedule::{counts, today};
use crate::task::Task;
use crate::when::LocalNow;
use crate::widget_text::{TimeFormat, format_hm, overdue_day_label, plain_title, row_date_label};

/// Which Todo view a widget shows. Today is the first; the others join as they are built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetView {
    /// Overdue, then what is due today.
    Today,
}

impl WidgetView {
    /// Reads the view's name as the widget stores it.
    pub fn parse(name: &str) -> Result<Self, TodoError> {
        match name {
            "today" => Ok(Self::Today),
            _ => Err(TodoError::Validation("That view is not available")),
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
    /// The view's name as asked for (`today`).
    pub view: &'static str,
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
    fn row(&self, task: &Task, overdue: bool, show_list: bool) -> WidgetRow {
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
            time: task
                .due_time
                .as_deref()
                .filter(|_| !overdue)
                .map(|t| format_hm(t, self.format)),
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

/// The List chip's text: emoji and name.
fn chip(emoji: Option<&str>, name: &str) -> String {
    match emoji {
        Some(e) if !e.is_empty() => format!("{e} {name}"),
        _ => name.to_owned(),
    }
}

/// Builds `view` for the widget as of `local`.
pub fn snapshot(
    conn: &Connection,
    view: WidgetView,
    local: &LocalNow,
) -> AppResult<WidgetSnapshot> {
    local.check()?;
    let format = TimeFormat::from_setting(
        settings::stored(conn, "core", "time_format")?
            .as_ref()
            .and_then(|v| v.as_str()),
    );
    let lists = list_repo::all(conn)?
        .into_iter()
        .map(|l| (l.id.clone(), chip(l.emoji.as_deref(), &l.name)))
        .collect();
    let cx = Context {
        local,
        format,
        lists,
    };
    match view {
        WidgetView::Today => today_snapshot(conn, &cx),
    }
}

fn today_snapshot(conn: &Connection, cx: &Context<'_>) -> AppResult<WidgetSnapshot> {
    let app = today(conn, cx.local)?;
    let (mut late, mut due) = (Vec::new(), Vec::new());
    for group in &app.groups {
        let overdue = group.key == GroupKey::Overdue;
        let rows = group.tasks.iter().map(|t| cx.row(t, overdue, true));
        if overdue {
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
    for (heading, tone, rows) in [
        ("Overdue", Tone::Overdue, late),
        ("Today", Tone::Normal, due),
    ] {
        if !rows.is_empty() {
            groups.push(WidgetGroup {
                heading: Some(heading.to_owned()),
                tone,
                rows,
            });
        }
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
    Ok(WidgetSnapshot {
        view: "today",
        title: "Today".to_owned(),
        count,
        groups,
        empty,
    })
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
