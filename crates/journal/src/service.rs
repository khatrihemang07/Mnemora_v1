//! The Journal's operations: rules, persistence and change-log records in one transaction.
//! Entry text and snapshots must never reach a log line.

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::error::{AppError, AppResult, ErrorCode};
use mnemora_core::util::{new_id, now_ms};
use rusqlite::Connection;

use crate::MODULE_ID;
use crate::entry::{
    Entry, NewEntry, Page, toggle_task, validate_page_date, validate_text, validate_written_zone,
};
use crate::{repo, words};

/// Change-log entity name for Entries.
const ENTITY: &str = "journal_entry";
/// Deleted Entries are kept this long (for Undo and recovery), then erased for good.
const PURGE_AFTER_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// Most Pages one request may return.
const MAX_PAGES: u32 = 100;
/// Changes sooner than this after the written instant are typo fixes, not edits.
const EDIT_GRACE_MS: i64 = 10 * 60 * 1000;

/// The "edited" instant after an update at `now`: kept once set, else set only past the grace.
fn edited_after_update(entry: &Entry, now: i64) -> Option<i64> {
    entry
        .edited_at
        .or_else(|| (now - entry.written_at > EDIT_GRACE_MS).then_some(now))
}

fn missing() -> AppError {
    AppError::new(ErrorCode::UnknownKey, "That Entry no longer exists")
}

fn record(
    conn: &Connection,
    entry: &Entry,
    op: ChangeOp,
    session_id: Option<&str>,
) -> AppResult<()> {
    changelog::record(
        conn,
        &Change {
            module: MODULE_ID.into(),
            entity: ENTITY.into(),
            entity_id: entry.id.clone(),
            op,
            session_id: session_id.map(str::to_owned),
            snapshot: serde_json::to_value(entry).map_err(|e| {
                AppError::new(ErrorCode::Internal, "Could not record the change").with_details(e)
            })?,
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
        word_count: words::word_count(&new.text),
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
    record(&tx, &entry, ChangeOp::Create, Some(&new.session_id))?;
    tx.commit()?;
    Ok(entry)
}

/// Replaces an Entry's text. Marks it edited only when more than 10 minutes have passed since
/// it was written.
pub fn update_entry(
    conn: &mut Connection,
    id: &str,
    text: String,
    session_id: &str,
) -> AppResult<Entry> {
    validate_text(&text)?;
    let tx = conn.transaction()?;
    let mut entry = repo::get(&tx, id)?.ok_or_else(missing)?;

    entry.word_count = words::word_count(&text);
    entry.body = text;
    let now = now_ms();
    entry.updated_at = now;
    entry.edited_at = edited_after_update(&entry, now);
    repo::update_body(&tx, &entry)?;
    record(&tx, &entry, ChangeOp::Update, Some(session_id))?;
    tx.commit()?;
    Ok(entry)
}

/// Ticks or unticks the checklist item on the 0-based source `line`. Never counts as an edit.
/// Each tick is its own change-log record: it has no editing session.
pub fn toggle_task_line(conn: &mut Connection, id: &str, line: usize) -> AppResult<Entry> {
    let tx = conn.transaction()?;
    let mut entry = repo::get(&tx, id)?.ok_or_else(missing)?;
    entry.body = toggle_task(&entry.body, line)?;
    entry.updated_at = now_ms();
    repo::update_body(&tx, &entry)?;
    record(&tx, &entry, ChangeOp::Tick, None)?;
    tx.commit()?;
    Ok(entry)
}

/// Moves an Entry to another Page (future dates are fine). Only the Page changes: the written
/// instant, text and "edited" mark stay as they were.
pub fn move_entry(conn: &mut Connection, id: &str, page_date: &str) -> AppResult<Entry> {
    validate_page_date(page_date)?;
    let tx = conn.transaction()?;
    let mut entry = repo::get(&tx, id)?.ok_or_else(missing)?;
    if entry.page_date == page_date {
        return Ok(entry);
    }
    page_date.clone_into(&mut entry.page_date);
    entry.updated_at = now_ms();
    repo::update_page_date(&tx, &entry)?;
    record(&tx, &entry, ChangeOp::Move, None)?;
    tx.commit()?;
    Ok(entry)
}

/// Soft-deletes an Entry: it leaves its Page at once and is erased after 30 days.
pub fn delete_entry(conn: &mut Connection, id: &str) -> AppResult<()> {
    let tx = conn.transaction()?;
    let entry = repo::get(&tx, id)?.ok_or_else(missing)?;
    repo::set_deleted_at(&tx, id, Some(now_ms()))?;
    record(&tx, &entry, ChangeOp::Delete, None)?;
    tx.commit()?;
    Ok(())
}

/// Brings a deleted Entry back. Pages order by the written instant, so it returns to its place.
pub fn restore_entry(conn: &mut Connection, id: &str) -> AppResult<Entry> {
    let tx = conn.transaction()?;
    let entry = repo::get_deleted(&tx, id)?.ok_or_else(missing)?;
    repo::set_deleted_at(&tx, id, None)?;
    record(&tx, &entry, ChangeOp::Restore, None)?;
    tx.commit()?;
    Ok(entry)
}

/// Permanently removes Entries deleted over 30 days ago, with their change-log records, in one
/// transaction. Returns how many Entries went. Run once at app start.
pub fn purge_deleted(conn: &mut Connection) -> AppResult<usize> {
    let tx = conn.transaction()?;
    let ids = repo::deleted_before(&tx, now_ms() - PURGE_AFTER_MS)?;
    for id in &ids {
        repo::remove(&tx, id)?;
    }
    changelog::purge(&tx, ENTITY, &ids)?;
    tx.commit()?;
    Ok(ids.len())
}

/// Dates in the given month (`month` is 1-12) that hold live Entries, oldest first.
pub fn month_days(conn: &Connection, year: u32, month: u32) -> AppResult<Vec<String>> {
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return Err(AppError::new(ErrorCode::Validation, "Not a valid month"));
    }
    // Dates are fixed-width text, so the month is a plain range; day 31 bounds every month.
    repo::dates_between(
        conn,
        &format!("{year:04}-{month:02}-01"),
        &format!("{year:04}-{month:02}-31"),
    )
}

/// Up to `limit` Pages, newest first, continuing before the `cursor` date when given.
pub fn list_pages(conn: &Connection, cursor: Option<&str>, limit: u32) -> AppResult<Vec<Page>> {
    if let Some(c) = cursor {
        validate_page_date(c)?;
    }
    let limit = limit.clamp(1, MAX_PAGES);
    Ok(repo::pages(conn, cursor, limit)?
        .into_iter()
        .map(|(date, entries)| Page {
            date,
            entry_count: u32::try_from(entries.len()).unwrap_or(u32::MAX),
            word_count: entries.iter().map(|e| e.word_count).sum(),
            entries,
        })
        .collect())
}
