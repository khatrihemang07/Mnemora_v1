//! The Task and Comment changes `todo_changes` reports for a range of days. A Task's changes are
//! its History lines (see `history`), read from the whole Task so each reads as it does there. A
//! Comment's creation is a row of its own, on its Task. A Task's title and Due date are as its
//! latest record has them.

use std::collections::BTreeSet;

use mnemora_core::changelog::{self, EntityRecord, Record};
use mnemora_core::error::AppResult;
use mnemora_core::recent::{self, PREVIEW_CHARS, Row, text_of};
use rusqlite::Connection;

use crate::comment_service::ENTITY as COMMENT;
use crate::history;
use crate::service::ENTITY as TASK;

/// The kinds `todo_changes` counts and filters by, in the order it lists them.
pub(crate) const KINDS: &[&str] = &[
    "created",
    "done",
    "wont",
    "skipped",
    "reopened",
    "moved",
    "due",
    "priority",
    "edited",
    "commented",
    "deleted",
    "restored",
    "repeat",
];

/// The kind a History line counts as, or `None` for a line the Tool does not report.
fn kind_of(history_kind: &str) -> Option<&'static str> {
    Some(match history_kind {
        "create" => "created",
        "done" => "done",
        "wont" => "wont",
        "skip" => "skipped",
        "reopen" => "reopened",
        "list" => "moved",
        "date" | "time" => "due",
        "priority" => "priority",
        "title" | "notes" => "edited",
        "delete" => "deleted",
        "restore" => "restored",
        "repeat" => "repeat",
        _ => return None,
    })
}

/// The rows of every Task and Comment change made in `[start, end)` (UTC milliseconds), in no
/// particular order: the Tool puts them in reading order.
pub(crate) fn rows(conn: &Connection, start: i64, end: i64) -> AppResult<Vec<Row>> {
    let changed = changelog::read_between(conn, crate::MODULE_ID, start, end)?;
    let mut walked = BTreeSet::new();
    let mut rows = Vec::new();
    for change in &changed {
        match change.entity.as_str() {
            TASK => {
                if walked.insert(change.entity_id.as_str()) {
                    rows.extend(task_rows(conn, &change.entity_id, start, end)?);
                }
            }
            COMMENT if change.record.op == "create" => {
                rows.extend(comment_row(conn, change)?);
            }
            _ => {}
        }
    }
    Ok(rows)
}

/// The rows one Task's changes in the range make. Every record of the Task is read, so the state
/// before each change is known; only the records inside the range make rows.
fn task_rows(conn: &Connection, id: &str, start: i64, end: i64) -> AppResult<Vec<Row>> {
    let records = changelog::read(conn, TASK, id)?;
    let (title, date) = standing(&records);
    let batches = history::lines_by_record(conn, &records)?;
    let mut rows = Vec::new();
    for (record, lines) in records.iter().zip(batches) {
        if record.at < start || record.at >= end || is_occurrence_copy(record) {
            continue;
        }
        // One editing session is one "Edited" row, however many of its fields it changed.
        let mut edited = false;
        for line in lines {
            let Some(kind) = kind_of(&line.kind) else {
                continue;
            };
            let change = if line.kind == "title" || line.kind == "notes" {
                if std::mem::replace(&mut edited, true) {
                    continue;
                }
                "Edited".to_owned()
            } else {
                line.text
            };
            rows.push(Row::of_record(
                id,
                record,
                kind,
                change,
                title.clone(),
                date.clone(),
            ));
        }
    }
    Ok(rows)
}

/// The occurrence a repeating Task leaves behind on completion. It is created already done, and
/// its completion is the series' own row (the `tick` on the series), so its creation adds none.
fn is_occurrence_copy(record: &Record) -> bool {
    record.op == "create" && text_of(&record.snapshot, "repeat_of").is_some()
}

/// A Comment's creation, on its Task's row: "Commented: " and a preview of its text.
fn comment_row(conn: &Connection, change: &EntityRecord) -> AppResult<Option<Row>> {
    let snapshot = &change.record.snapshot;
    let (Some(task), Some(body)) = (text_of(snapshot, "task_id"), text_of(snapshot, "body")) else {
        return Ok(None);
    };
    let (title, date) = standing(&changelog::read(conn, TASK, task)?);
    let change_line = format!("Commented: {}", recent::preview(body, PREVIEW_CHARS));
    Ok(Some(Row::of_record(
        task,
        &change.record,
        "commented",
        change_line,
        title,
        date,
    )))
}

/// A Task's title and Due date as its latest record has them. Empty when it has no record.
fn standing(records: &[Record]) -> (String, Option<String>) {
    records.last().map_or_else(
        || (String::new(), None),
        |latest| {
            (
                text_of(&latest.snapshot, "title")
                    .unwrap_or_default()
                    .to_owned(),
                text_of(&latest.snapshot, "due_date").map(str::to_owned),
            )
        },
    )
}
