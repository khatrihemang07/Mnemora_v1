//! The Journal's operations: rules, persistence and change-log records in one transaction.
//! Entry text and snapshots must never reach a log line.

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::error::AppResult;
use mnemora_core::search::plain;
use mnemora_core::util::{new_id, now_ms};
use rusqlite::Connection;

use crate::MODULE_ID;
use crate::entry::{
    Entry, NewEntry, Page, PageDirection, toggle_task, validate_page_date, validate_text,
    validate_written_zone,
};
use crate::error::JournalError;
use crate::repo::{self, Window};

/// Change-log entity name for Entries.
const ENTITY: &str = "journal_entry";
/// Most Pages one request may return.
const MAX_PAGES: u32 = 100;
/// Changes sooner than this after the written instant are typo fixes, not edits.
const EDIT_GRACE_MS: i64 = 10 * 60 * 1000;

/// The "edited" instant after an update at `now`: the last edit, so it moves forward with every
/// update past the grace; within the grace the existing value stays.
fn edited_after_update(entry: &Entry, now: i64) -> Option<i64> {
    (now - entry.written_at > EDIT_GRACE_MS)
        .then_some(now)
        .or(entry.edited_at)
}

/// Writes a change-log record. The snapshot is the Entry plus its `deleted_at`, which the Entry
/// type does not carry: a delete record shows when it was deleted, every other one shows null.
fn record(
    conn: &Connection,
    entry: &Entry,
    deleted_at: Option<i64>,
    op: ChangeOp,
    session_id: Option<&str>,
) -> AppResult<()> {
    let mut snapshot = serde_json::to_value(entry).map_err(JournalError::Snapshot)?;
    if let Some(fields) = snapshot.as_object_mut() {
        fields.insert("deleted_at".into(), deleted_at.into());
    }
    changelog::record(
        conn,
        &Change {
            module: MODULE_ID.into(),
            entity: ENTITY.into(),
            entity_id: entry.id.clone(),
            op,
            session_id: session_id.map(str::to_owned),
            snapshot,
        },
    )
}

/// Writes a new Entry on its Page. The written instant is now.
pub fn create_entry(conn: &mut Connection, new: NewEntry) -> AppResult<Entry> {
    validate_text(&new.text)?;
    validate_page_date(&new.page_date)?;
    validate_written_zone(&new.written_tz, new.written_offset_min)?;

    let now = now_ms();
    let entry = Entry {
        id: new_id(),
        page_date: new.page_date,
        word_count: plain::word_count(&new.text),
        body: new.text,
        written_at: now,
        written_offset_min: new.written_offset_min,
        written_tz: new.written_tz,
        edited_at: None,
        created_at: now,
        updated_at: now,
    };
    let tx = conn.transaction()?;
    repo::insert(&tx, &entry)?;
    record(&tx, &entry, None, ChangeOp::Create, Some(&new.session_id))?;
    tx.commit()?;
    Ok(entry)
}

/// Replaces an Entry's text. Marks it edited (at the latest update) only when more than 10
/// minutes have passed since it was written.
pub fn update_entry(
    conn: &mut Connection,
    id: &str,
    text: String,
    session_id: &str,
) -> AppResult<Entry> {
    validate_text(&text)?;
    let tx = conn.transaction()?;
    let mut entry = repo::get(&tx, id)?.ok_or(JournalError::EntryNotFound)?;

    entry.word_count = plain::word_count(&text);
    entry.body = text;
    let now = now_ms();
    entry.updated_at = now;
    entry.edited_at = edited_after_update(&entry, now);
    repo::update_body(&tx, &entry)?;
    record(&tx, &entry, None, ChangeOp::Update, Some(session_id))?;
    tx.commit()?;
    Ok(entry)
}

/// Ticks or unticks the checklist item on the 0-based source `line`. Never counts as an edit.
/// Each tick is its own change-log record: it has no editing session.
pub fn toggle_task_line(conn: &mut Connection, id: &str, line: usize) -> AppResult<Entry> {
    let tx = conn.transaction()?;
    let mut entry = repo::get(&tx, id)?.ok_or(JournalError::EntryNotFound)?;
    entry.body = toggle_task(&entry.body, line)?;
    entry.updated_at = now_ms();
    repo::update_body(&tx, &entry)?;
    record(&tx, &entry, None, ChangeOp::Tick, None)?;
    tx.commit()?;
    Ok(entry)
}

/// Moves an Entry to another Page (future dates are fine). Only the Page changes: the written
/// instant, text and "edited" mark stay as they were.
pub fn move_entry(conn: &mut Connection, id: &str, page_date: &str) -> AppResult<Entry> {
    validate_page_date(page_date)?;
    let tx = conn.transaction()?;
    let mut entry = repo::get(&tx, id)?.ok_or(JournalError::EntryNotFound)?;
    if entry.page_date == page_date {
        return Ok(entry);
    }
    page_date.clone_into(&mut entry.page_date);
    entry.updated_at = now_ms();
    repo::update_page_date(&tx, &entry)?;
    record(&tx, &entry, None, ChangeOp::Move, None)?;
    tx.commit()?;
    Ok(entry)
}

/// Soft-deletes an Entry: it leaves its Page at once and stays as its own tombstone.
pub fn delete_entry(conn: &mut Connection, id: &str) -> AppResult<()> {
    let tx = conn.transaction()?;
    let entry = repo::get(&tx, id)?.ok_or(JournalError::EntryNotFound)?;
    let now = now_ms();
    repo::set_deleted_at(&tx, id, Some(now), now)?;
    record(&tx, &entry, Some(now), ChangeOp::Delete, None)?;
    tx.commit()?;
    Ok(())
}

/// Brings a deleted Entry back. Pages order by the written instant, so it returns to its place.
pub fn restore_entry(conn: &mut Connection, id: &str) -> AppResult<Entry> {
    let tx = conn.transaction()?;
    let mut entry = repo::get_deleted(&tx, id)?.ok_or(JournalError::EntryNotFound)?;
    let now = now_ms();
    repo::set_deleted_at(&tx, id, None, now)?;
    entry.updated_at = now;
    record(&tx, &entry, None, ChangeOp::Restore, None)?;
    tx.commit()?;
    Ok(entry)
}

/// Dates in the given month (`month` is 1-12) that hold live Entries, oldest first.
pub fn month_days(conn: &Connection, year: u32, month: u32) -> AppResult<Vec<String>> {
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return Err(JournalError::Validation("Not a valid month").into());
    }
    // Dates are fixed-width text, so the month is a plain range; day 31 bounds every month.
    repo::dates_between(
        conn,
        &format!("{year:04}-{month:02}-01"),
        &format!("{year:04}-{month:02}-31"),
    )
}

/// Up to `limit` Pages, newest first, picked relative to the `cursor` date by `direction`: older
/// than it, starting at it, or the nearest newer ones (see [`PageDirection`]).
pub fn list_pages(
    conn: &Connection,
    cursor: Option<&str>,
    limit: u32,
    direction: PageDirection,
) -> AppResult<Vec<Page>> {
    if let Some(c) = cursor {
        validate_page_date(c)?;
    }
    let window = match (direction, cursor) {
        (PageDirection::Older | PageDirection::From, None) => Window::Newest,
        (PageDirection::Older, Some(c)) => Window::Before(c),
        (PageDirection::From, Some(c)) => Window::AtOrBefore(c),
        (PageDirection::Newer, Some(c)) => Window::After(c),
        (PageDirection::Newer, None) => {
            return Err(JournalError::Validation("Newer Pages need a date to start from").into());
        }
    };
    let limit = limit.clamp(1, MAX_PAGES);
    Ok(repo::pages(conn, window, limit)?
        .into_iter()
        .map(|(date, entries)| Page {
            date,
            entry_count: u32::try_from(entries.len()).unwrap_or(u32::MAX),
            word_count: entries.iter().map(|e| e.word_count).sum(),
            entries,
        })
        .collect())
}
