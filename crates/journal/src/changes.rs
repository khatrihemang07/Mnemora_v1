//! The Entry changes `journal_changes` reports for a range of days: an Entry written, edited or
//! deleted, read from the change log (ADR 0008). An editing session is one record, so it is one
//! "Edited" row however many saves it held.

use mnemora_core::changelog;
use mnemora_core::error::AppResult;
use mnemora_core::recent::{self, PREVIEW_CHARS, Row, text_of};
use rusqlite::Connection;
use serde_json::Value;

use crate::MODULE_ID;
use crate::service::ENTITY;

/// The kinds `journal_changes` counts and filters by, in the order it lists them.
pub(crate) const KINDS: &[&str] = &["written", "edited", "deleted"];

/// The rows of every Entry change made in `[start, end)` (UTC milliseconds), in no particular
/// order: the Tool puts them in reading order. A deleted Entry keeps its preview.
pub(crate) fn rows(conn: &Connection, start: i64, end: i64) -> AppResult<Vec<Row>> {
    let changed = changelog::read_between(conn, MODULE_ID, start, end)?;
    let mut rows = Vec::new();
    for change in changed.iter().filter(|c| c.entity == ENTITY) {
        let record = &change.record;
        let snapshot = &record.snapshot;
        let (kind, line) = match record.op.as_str() {
            "create" => ("written", written(snapshot)),
            "update" => ("edited", "Edited".to_owned()),
            "delete" => ("deleted", "Deleted".to_owned()),
            _ => continue,
        };
        let preview = recent::preview(text_of(snapshot, "body").unwrap_or_default(), PREVIEW_CHARS);
        let page_date = text_of(snapshot, "page_date")
            .unwrap_or_default()
            .to_owned();
        rows.push(Row::of_record(
            &change.entity_id,
            record,
            kind,
            line,
            preview,
            Some(page_date),
        ));
    }
    Ok(rows)
}

/// "Written, 42 words", from the word count the Entry's snapshot holds.
fn written(snapshot: &Value) -> String {
    let words = snapshot
        .get("word_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let noun = if words == 1 { "word" } else { "words" };
    format!("Written, {words} {noun}")
}
