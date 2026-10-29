//! The Todo Module's read-only Tools for the AI (ADR 0002 amendment, ADR 0018): list Tasks by state,
//! List, due date, priority and closing day; read one in full; search with the app's own Search;
//! and say what changed in a range of days. They never write, skip deleted rows and return refusals
//! as data. Closing days and local times are read in the device's zone, as the client sends it.

use mnemora_core::batch;
use mnemora_core::error::AppResult;
use mnemora_core::local_time;
use mnemora_core::recent;
use mnemora_core::tool::{
    Args, Calendar, Item, Kind, Output, Param, Spec, Tool, count, cut, fit_rows, number,
};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};

use crate::list::name_key;
use crate::repo::{COLUMNS, from_row};
use crate::steps::checklist_items;
use crate::task::{Task, TaskState};
use crate::{changes, comment_repo, history, list_repo};

/// The Module's id, as the change log and the AI name it.
const MODULE_ID: &str = "todo";

/// Bytes of a Task's notes one read batch returns; the next offset continues them.
const READ_BATCH_BYTES: usize = 8 * 1024;
/// Rows a list returns by default, and at most.
const LIST_DEFAULT: usize = 30;
const LIST_MAX: i64 = 100;
/// Rows a search returns by default, and at most.
const SEARCH_DEFAULT: usize = 10;
const SEARCH_MAX: i64 = 25;

const STATES: &[&str] = &["open", "done", "wont", "any"];

const LIST_PARAMS: &[Param] = &[
    Param {
        name: "state",
        kind: Kind::Choice(STATES),
        required: false,
        about: "open, done, wont (Won't do) or any. With closed_from or closed_to and no state: done and wont. Otherwise open.",
    },
    Param {
        name: "list",
        kind: Kind::Text { max_chars: 60 },
        required: false,
        about: "A List's name, or inbox for Tasks with no List.",
    },
    Param {
        name: "due_from",
        kind: Kind::Date,
        required: false,
        about: "Earliest due date, YYYY-MM-DD.",
    },
    Param {
        name: "due_to",
        kind: Kind::Date,
        required: false,
        about: "Latest due date, YYYY-MM-DD.",
    },
    Param {
        name: "priority",
        kind: Kind::Integer { min: 1, max: 4 },
        required: false,
        about: "1 is most urgent, 4 is none.",
    },
    Param {
        name: "closed_from",
        kind: Kind::Date,
        required: false,
        about: "First day closed, YYYY-MM-DD, in the device's time zone.",
    },
    Param {
        name: "closed_to",
        kind: Kind::Date,
        required: false,
        about: "Last day closed, YYYY-MM-DD, in the device's time zone.",
    },
    Param {
        name: "offset",
        kind: Kind::Integer {
            min: 0,
            max: 1_000_000,
        },
        required: false,
        about: "Rows to skip: the next_offset of the batch before, 0 for the first.",
    },
    Param {
        name: "limit",
        kind: Kind::Integer {
            min: 1,
            max: LIST_MAX,
        },
        required: false,
        about: "Rows in one batch, 30 by default.",
    },
];

const READ_PARAMS: &[Param] = &[
    Param {
        name: "id",
        kind: Kind::Text { max_chars: 64 },
        required: true,
        about: "The Task's id, as a list or search returned it.",
    },
    Param {
        name: "steps",
        kind: Kind::Flag,
        required: false,
        about: "Include the Task's checklist steps.",
    },
    Param {
        name: "comments",
        kind: Kind::Flag,
        required: false,
        about: "Include the Task's Comments.",
    },
    Param {
        name: "history",
        kind: Kind::Flag,
        required: false,
        about: "Include the Task's History.",
    },
    Param {
        name: "offset",
        kind: Kind::Integer {
            min: 0,
            max: 10_000_000,
        },
        required: false,
        about: "Bytes of the notes to skip: the next_offset of the batch before, 0 for the first.",
    },
];

const SEARCH_PARAMS: &[Param] = &[
    Param {
        name: "query",
        kind: Kind::Text { max_chars: 200 },
        required: true,
        about: "Words to find in titles, notes or Comments. Quotes make a phrase.",
    },
    Param {
        name: "state",
        kind: Kind::Choice(STATES),
        required: false,
        about: "open, done, wont (Won't do) or any (the default).",
    },
    Param {
        name: "list",
        kind: Kind::Text { max_chars: 60 },
        required: false,
        about: "A List's name, or inbox for Tasks with no List.",
    },
    Param {
        name: "offset",
        kind: Kind::Integer {
            min: 0,
            max: 1_000_000,
        },
        required: false,
        about: "Results to skip: the next_offset of the batch before, 0 for the first.",
    },
    Param {
        name: "limit",
        kind: Kind::Integer {
            min: 1,
            max: SEARCH_MAX,
        },
        required: false,
        about: "Results in one batch, 10 by default.",
    },
];

/// The changes Tool's parameters: the shared range, paging and limit (`recent`), and the kinds this
/// Module names.
const CHANGES_PARAMS: &[Param] = &[
    recent::FROM,
    recent::TO,
    Param {
        name: "kinds",
        kind: Kind::Text { max_chars: 200 },
        required: false,
        about: "Only these kinds, comma-separated: created, done, wont, skipped, reopened, moved, due, priority, edited, commented, deleted, restored, repeat. Counts still cover the whole range.",
    },
    recent::OFFSET,
    recent::LIMIT,
];

/// The Tools the Todo Module offers.
pub(crate) fn all() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(List),
        Box::new(Read),
        Box::new(Search),
        Box::new(Changes),
    ]
}

fn unreadable() -> Output {
    Output::error("The Todo Module could not be read just now.")
}

/// Which Tasks a `list` argument picks.
enum ListPick {
    /// No List was named: every Task.
    Any,
    /// The Inbox: Tasks with no List.
    Inbox,
    /// One List, by id.
    List(String),
}

/// The List a `list` argument names. An unknown name is an error.
fn list_filter(conn: &Connection, name: Option<&str>) -> Result<ListPick, Output> {
    let Some(name) = name else {
        return Ok(ListPick::Any);
    };
    if name.trim().eq_ignore_ascii_case("inbox") {
        return Ok(ListPick::Inbox);
    }
    let found = conn
        .query_row(
            "SELECT id FROM todo_lists WHERE name_key = ?1 AND deleted_at IS NULL",
            [name_key(name)],
            |r| r.get::<_, String>(0),
        )
        .optional();
    match found {
        Ok(Some(id)) => Ok(ListPick::List(id)),
        Ok(None) => Err(Output::error("No List has that name.")),
        Err(_) => Err(unreadable()),
    }
}

/// The List names by id, for the rows.
fn list_names(conn: &Connection) -> Result<std::collections::HashMap<String, String>, Output> {
    list_repo::all(conn)
        .map(|lists| lists.into_iter().map(|l| (l.id, l.name)).collect())
        .map_err(|_| unreadable())
}

fn list_label(names: &std::collections::HashMap<String, String>, task: &Task) -> String {
    match &task.list_id {
        None => "Inbox".to_owned(),
        Some(id) => names.get(id).cloned().unwrap_or_else(|| "Inbox".to_owned()),
    }
}

/// A Task's row. Each timestamp is epoch milliseconds, with a `_local` reading in the device's zone.
fn row_json(task: &Task, list: &str, zone: &str) -> Value {
    json!({
        "id": task.id,
        "title": task.title,
        "list": list,
        "state": task.state.as_str(),
        "due": task.due_date,
        "priority": task.priority,
        "closed_at": task.closed_at,
        "closed_at_local": task.closed_at.and_then(|ms| local_time::minute(ms, zone)),
    })
}

/// The rows of `tasks` for a list or search, each with its Comment hints (see `with_comment_hints`).
fn hinted_rows<'a>(
    conn: &Connection,
    tasks: impl IntoIterator<Item = &'a Task>,
    names: &std::collections::HashMap<String, String>,
    zone: &str,
) -> AppResult<Vec<Value>> {
    tasks
        .into_iter()
        .map(|task| {
            let mut row = row_json(task, &list_label(names, task), zone);
            with_comment_hints(conn, &task.id, &mut row, zone)?;
            Ok(row)
        })
        .collect()
}

/// Adds to a row how many live Comments its Task has and when the newest was written, with a local
/// reading. A Task with no live Comment gets none of the three fields, which keeps rows small.
fn with_comment_hints(
    conn: &Connection,
    task_id: &str,
    row: &mut Value,
    zone: &str,
) -> AppResult<()> {
    if let Some((count, newest)) = comment_repo::hint(conn, task_id)? {
        row["comments"] = json!(count);
        row["last_comment_at"] = json!(newest);
        row["last_comment_at_local"] = json!(local_time::minute(newest, zone));
    }
    Ok(())
}

/// The instants a closing-day range covers in the device's zone: from the start of its first day
/// to the end of its last. A zone or day that cannot be read is a refusal.
fn closed_bounds(
    from: Option<&str>,
    to: Option<&str>,
    zone: &str,
) -> Result<(Option<i64>, Option<i64>), Output> {
    let refused = || Output::error("The closing days could not be read in the device's time zone.");
    let from = from
        .map(|day| local_time::day_start(day, zone).ok_or_else(refused))
        .transpose()?;
    let to = to
        .map(|day| local_time::day_end(day, zone).ok_or_else(refused))
        .transpose()?;
    Ok((from, to))
}

fn item(task: &Task) -> Item {
    Item {
        id: task.id.clone(),
        module: MODULE_ID,
        title: cut(&task.title, 120).to_owned(),
        date: task.due_date.clone(),
    }
}

struct List;

impl Tool for List {
    fn spec(&self) -> Spec {
        Spec {
            name: "todo_list",
            about: "List Tasks by state, List, due date, priority and the day they were closed. With closed_from or closed_to it lists done and Won't do Tasks, newest close first. A row with comments above 0 has updates and progress notes: read them with todo_read and comments true. last_comment_at is when its newest live Comment was written. Timestamps are epoch milliseconds (UTC); each has a _local twin, YYYY-MM-DD HH:MM in the device's time zone, or null. Each batch has shown (its rows) and total (the rows the query matches). If next_offset is not null the list is incomplete: fetch the next batch with that offset before summarising.",
            params: LIST_PARAMS,
        }
    }

    fn run(&self, conn: &Connection, args: &Args, calendar: &Calendar) -> Output {
        let (closed_from, closed_to) = (args.text("closed_from"), args.text("closed_to"));
        let closed = closed_from.is_some() || closed_to.is_some();
        // A closing-day question is about Tasks that were done or declined, unless a state is named.
        let explicit = args.text("state");
        let closed_only = i64::from(closed && explicit.is_none());
        let state = explicit.unwrap_or(if closed { "any" } else { "open" });
        let state = (state != "any").then_some(state);
        let (closed_from_ms, closed_to_ms) =
            match closed_bounds(closed_from, closed_to, &calendar.time_zone) {
                Ok(bounds) => bounds,
                Err(refusal) => return refusal,
            };
        let list = match list_filter(conn, args.text("list")) {
            Ok(list) => list,
            Err(refusal) => return refusal,
        };
        let (list_on, list_id) = match &list {
            ListPick::Any => (0_i64, None),
            ListPick::Inbox => (1, None),
            ListPick::List(id) => (1, Some(id.as_str())),
        };
        let offset = count(args.int("offset"), 0);
        let limit = count(args.int("limit"), LIST_DEFAULT);
        let priority = args.int("priority");
        let (due_from, due_to) = (args.text("due_from"), args.text("due_to"));
        let names = match list_names(conn) {
            Ok(names) => names,
            Err(refusal) => return refusal,
        };
        // A closing-day list reads as a log, newest close first; any other list reads by due date.
        let order = if closed {
            "closed_at DESC, sort_key"
        } else {
            "(state = 'open') DESC, due_date IS NULL, due_date, closed_at DESC, sort_key"
        };
        // One filter for the count of every match and for the batch, so `total` is what paging runs over.
        let filter = "deleted_at IS NULL
               AND (?1 IS NULL OR state = ?1)
               AND (?2 = 0 OR list_id IS ?3)
               AND (?4 IS NULL OR due_date >= ?4)
               AND (?5 IS NULL OR due_date <= ?5)
               AND (?6 IS NULL OR priority = ?6)
               AND (?7 IS NULL OR closed_at >= ?7)
               AND (?8 IS NULL OR closed_at < ?8)
               AND (?11 = 0 OR state IN ('done', 'wont'))";
        let sql = format!(
            "SELECT {COLUMNS} FROM todo_tasks WHERE {filter} ORDER BY {order} LIMIT ?9 OFFSET ?10"
        );
        let bound = rusqlite::params![
            state,
            list_on,
            list_id,
            due_from,
            due_to,
            priority,
            closed_from_ms,
            closed_to_ms,
            number(limit + 1),
            number(offset),
            closed_only,
        ];
        let counted = conn.query_row(
            &format!("SELECT COUNT(*) FROM todo_tasks WHERE {filter}"),
            bound,
            |r| r.get::<_, i64>(0),
        );
        let Ok(total) = counted else {
            return unreadable();
        };
        let total = count(Some(total), 0);
        let fetched = conn.prepare(&sql).and_then(|mut stmt| {
            stmt.query_map(bound, from_row)?
                .collect::<rusqlite::Result<Vec<Task>>>()
        });
        let Ok(mut tasks) = fetched else {
            return unreadable();
        };
        let more = tasks.len() > limit;
        tasks.truncate(limit);
        let zone = calendar.time_zone.as_str();
        let Ok(rows) = hinted_rows(conn, &tasks, &names, zone) else {
            return unreadable();
        };
        let (kept, cut_short) = fit_rows(rows);
        let shown = kept.len();
        let next = (more || cut_short).then_some(offset + shown);
        let items = tasks.iter().take(shown).map(item).collect();
        let mut body = json!({ "tasks": kept, "next_offset": next });
        if let Some(fields) = body.as_object_mut() {
            fields.extend(batch::fields(offset, shown, total, next, "tasks", None));
        }
        Output::ok(
            body,
            match shown {
                0 => "Listed tasks, none".to_owned(),
                n => format!("Listed tasks, {n} tasks"),
            },
            items,
        )
    }
}

struct Read;

impl Tool for Read {
    fn spec(&self) -> Spec {
        Spec {
            name: "todo_read",
            about: "Read one Task in full: its notes, and optionally steps, Comments and History. Comments are its updates and progress notes; set comments true when a list or search row shows comments above 0. Timestamps are epoch milliseconds (UTC); each has a _local twin, YYYY-MM-DD HH:MM in the device's time zone, or null. Long notes come in batches: shown and total are bytes. If next_offset is not null the notes are incomplete: fetch the next batch with that offset before summarising.",
            params: READ_PARAMS,
        }
    }

    fn run(&self, conn: &Connection, args: &Args, calendar: &Calendar) -> Output {
        let zone = calendar.time_zone.as_str();
        let id = args.text("id").unwrap_or_default();
        let task = match crate::repo::get(conn, id) {
            Ok(Some(task)) => task,
            Ok(None) => return Output::error("No Task has that id."),
            Err(_) => return unreadable(),
        };
        let names = match list_names(conn) {
            Ok(names) => names,
            Err(refusal) => return refusal,
        };
        let offset = count(args.int("offset"), 0);
        if offset > task.notes.len() {
            return Output::error("The offset is past the end of the notes.");
        }
        let mut start = offset;
        while !task.notes.is_char_boundary(start) {
            start += 1;
        }
        let rest = &task.notes[start..];
        let page = cut(rest, READ_BATCH_BYTES);
        let next = (page.len() < rest.len()).then_some(start + page.len());
        let mut body = row_json(&task, &list_label(&names, &task), zone);
        body["notes"] = json!(page);
        body["next_offset"] = json!(next);
        if let Some(fields) = body.as_object_mut() {
            fields.extend(batch::byte_fields(
                start,
                page.len(),
                task.notes.len(),
                next,
            ));
        }
        body["created_at"] = json!(task.created_at);
        body["created_at_local"] = json!(local_time::minute(task.created_at, zone));
        if args.flag("steps") == Some(true) {
            let steps: Vec<Value> = checklist_items(&task.notes)
                .into_iter()
                .map(|(text, done)| json!({ "text": text, "done": done }))
                .collect();
            body["steps"] = json!(steps);
        }
        if args.flag("comments") == Some(true) {
            let comments: Vec<Value> = match comment_repo::for_task(conn, &task.id) {
                Ok(list) => list
                    .into_iter()
                    .map(|c| {
                        json!({
                            "body": c.body,
                            "created_at": c.created_at,
                            "created_at_local": local_time::minute(c.created_at, zone),
                        })
                    })
                    .collect(),
                Err(_) => return unreadable(),
            };
            let (kept, cut_short) = fit_rows(comments);
            body["comments"] = json!(kept);
            body["comments_truncated"] = json!(cut_short);
        }
        if args.flag("history") == Some(true) {
            let Ok(lines) = history::task_history(conn, &task.id) else {
                return unreadable();
            };
            let rows: Vec<Value> = lines
                .iter()
                .map(|l| {
                    json!({
                        "at": l.at,
                        "at_local": local_time::minute(l.at, zone),
                        "kind": l.kind,
                        "text": l.text,
                    })
                })
                .collect();
            let (kept, cut_short) = fit_rows(rows);
            body["history"] = json!(kept);
            body["history_truncated"] = json!(cut_short);
        }
        let summary = match task.state {
            TaskState::Open => "Read a task, open".to_owned(),
            TaskState::Done => "Read a task, done".to_owned(),
            TaskState::Wont => "Read a task, won't do".to_owned(),
        };
        Output::ok(body, summary, vec![item(&task)])
    }
}

struct Search;

impl Tool for Search {
    fn spec(&self) -> Spec {
        Spec {
            name: "todo_search",
            about: "Find Tasks with the app's Search, open ones first. A row with comments above 0 has updates and progress notes: read them with todo_read and comments true. last_comment_at is when its newest live Comment was written. Timestamps are epoch milliseconds (UTC); each has a _local twin, YYYY-MM-DD HH:MM in the device's time zone, or null. Each batch has shown (its rows) and total (the rows the query matches). If next_offset is not null the list is incomplete: fetch the next batch with that offset before summarising.",
            params: SEARCH_PARAMS,
        }
    }

    fn run(&self, conn: &Connection, args: &Args, calendar: &Calendar) -> Output {
        let query = args.text("query").unwrap_or_default();
        let state = args.text("state").unwrap_or("any");
        let list = match list_filter(conn, args.text("list")) {
            Ok(list) => list,
            Err(refusal) => return refusal,
        };
        let offset = count(args.int("offset"), 0);
        let limit = count(args.int("limit"), SEARCH_DEFAULT);
        let names = match list_names(conn) {
            Ok(names) => names,
            Err(refusal) => return refusal,
        };
        let Ok(results) = crate::search(conn, query) else {
            return Output::error("Search could not run. Check the words.");
        };
        let matching: Vec<&Task> = results
            .exact
            .iter()
            .chain(results.similar.iter())
            .map(|hit| &hit.task)
            .filter(|task| state == "any" || task.state.as_str() == state)
            .filter(|task| match &list {
                ListPick::Any => true,
                ListPick::Inbox => task.list_id.is_none(),
                ListPick::List(id) => task.list_id.as_deref() == Some(id.as_str()),
            })
            .collect();
        let total = matching.len();
        let page: Vec<&Task> = matching.into_iter().skip(offset).take(limit).collect();
        let zone = calendar.time_zone.as_str();
        let Ok(rows) = hinted_rows(conn, page.iter().copied(), &names, zone) else {
            return unreadable();
        };
        let (kept, _) = fit_rows(rows);
        let shown = kept.len();
        let items = page.iter().take(shown).map(|t| item(t)).collect();
        let next = (offset + shown < total).then_some(offset + shown);
        let mut body = json!({ "tasks": kept, "next_offset": next });
        if let Some(fields) = body.as_object_mut() {
            fields.extend(batch::fields(offset, shown, total, next, "matches", None));
        }
        Output::ok(body, format!("Searched tasks, {total} found"), items)
    }
}

struct Changes;

impl Tool for Changes {
    fn spec(&self) -> Spec {
        Spec {
            name: "todo_changes",
            about: "Task creations, completions, reschedules, moves and Comments written in a date range (a Comment reads as commented, with its text). Use for what was done or updated. Days are local, in the device's time zone: rows are grouped by day, newest day first, and each day's rows oldest first. Each row has id, at (HH:MM, local time), change and title. Counts cover the whole range by kind. A repeating Task's completion reads as its occurrence, such as 'Done (occurrence of 18 Oct)'. Each batch has shown (its rows) and total (the rows the query matches). If next_offset is not null the list is incomplete: fetch the next batch with that offset before summarising.",
            params: CHANGES_PARAMS,
        }
    }

    fn run(&self, conn: &Connection, args: &Args, calendar: &Calendar) -> Output {
        recent::run(args, calendar, MODULE_ID, changes::KINDS, |start, end| {
            changes::rows(conn, start, end).map_err(|_| unreadable())
        })
    }
}
