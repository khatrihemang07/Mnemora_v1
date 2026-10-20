//! The Journal's operations: rules, persistence and change-log records in one transaction.
//! Entry text and snapshots must never reach a log line.

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::error::{AppError, AppResult, ErrorCode};
use mnemora_core::util::{new_id, now_ms};
use rusqlite::Connection;

use crate::MODULE_ID;
use crate::entry::{
    Entry, NewEntry, Page, validate_page_date, validate_text, validate_written_zone,
};
use crate::{repo, words};

/// Change-log entity name for Entries.
const ENTITY: &str = "journal_entry";
/// Most Pages one request may return.
const MAX_PAGES: u32 = 100;

fn record(conn: &Connection, entry: &Entry, op: ChangeOp, session_id: &str) -> AppResult<()> {
    changelog::record(
        conn,
        &Change {
            module: MODULE_ID.into(),
            entity: ENTITY.into(),
            entity_id: entry.id.clone(),
            op,
            session_id: Some(session_id.to_owned()),
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
    record(&tx, &entry, ChangeOp::Create, &new.session_id)?;
    tx.commit()?;
    Ok(entry)
}

/// Replaces an Entry's text. Setting `edited_at` (the 10-minute rule) will go here.
pub fn update_entry(
    conn: &mut Connection,
    id: &str,
    text: String,
    session_id: &str,
) -> AppResult<Entry> {
    validate_text(&text)?;
    let tx = conn.transaction()?;
    let mut entry = repo::get(&tx, id)?
        .ok_or_else(|| AppError::new(ErrorCode::UnknownKey, "That Entry no longer exists"))?;

    entry.word_count = words::word_count(&text);
    entry.body = text;
    entry.updated_at = now_ms();
    repo::update_body(&tx, id, &entry.body, entry.word_count, entry.updated_at)?;
    record(&tx, &entry, ChangeOp::Update, session_id)?;
    tx.commit()?;
    Ok(entry)
}

/// Up to `limit` Pages, newest first, continuing before the `cursor` date when given.
pub fn list_pages(conn: &Connection, cursor: Option<&str>, limit: u32) -> AppResult<Vec<Page>> {
    if let Some(c) = cursor {
        validate_page_date(c)?;
    }
    let limit = limit.clamp(1, MAX_PAGES);
    Ok(repo::pages(conn, cursor, limit)?
        .into_iter()
        .map(|(date, entries)| Page { date, entries })
        .collect())
}
