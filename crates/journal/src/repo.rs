//! SQL for the Entries table. No rules here; see `service.rs` and `entry.rs`.

use mnemora_core::error::AppResult;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::entry::Entry;

const COLUMNS: &str = "id, page_date, body, written_at, written_offset_min, written_tz,
    edited_at, word_count, created_at, updated_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: r.get(0)?,
        page_date: r.get(1)?,
        body: r.get(2)?,
        written_at: r.get(3)?,
        written_offset_min: r.get(4)?,
        written_tz: r.get(5)?,
        edited_at: r.get(6)?,
        word_count: r.get(7)?,
        created_at: r.get(8)?,
        updated_at: r.get(9)?,
    })
}

pub fn insert(conn: &Connection, e: &Entry) -> AppResult<()> {
    conn.execute(
        "INSERT INTO journal_entries(id, page_date, body, written_at, written_offset_min,
             written_tz, edited_at, word_count, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            e.id,
            e.page_date,
            e.body,
            e.written_at,
            e.written_offset_min,
            e.written_tz,
            e.edited_at,
            e.word_count,
            e.created_at,
            e.updated_at
        ],
    )?;
    Ok(())
}

/// A live (not deleted) Entry.
pub fn get(conn: &Connection, id: &str) -> AppResult<Option<Entry>> {
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM journal_entries WHERE id = ?1 AND deleted_at IS NULL"),
            [id],
            from_row,
        )
        .optional()?)
}

/// A soft-deleted Entry.
pub fn get_deleted(conn: &Connection, id: &str) -> AppResult<Option<Entry>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM journal_entries WHERE id = ?1 AND deleted_at IS NOT NULL"
            ),
            [id],
            from_row,
        )
        .optional()?)
}

/// Sets or clears `deleted_at`.
pub fn set_deleted_at(conn: &Connection, id: &str, at: Option<i64>) -> AppResult<()> {
    conn.execute(
        "UPDATE journal_entries SET deleted_at = ?2 WHERE id = ?1",
        params![id, at],
    )?;
    Ok(())
}

/// Ids of Entries soft-deleted before `cutoff`.
pub fn deleted_before(conn: &Connection, cutoff: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM journal_entries WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
    )?;
    Ok(stmt
        .query_map([cutoff], |r| r.get(0))?
        .collect::<Result<_, _>>()?)
}

/// Permanently removes the Entry.
pub fn remove(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM journal_entries WHERE id = ?1", [id])?;
    Ok(())
}

/// Writes the body and the fields an update changes.
pub fn update_body(conn: &Connection, e: &Entry) -> AppResult<()> {
    conn.execute(
        "UPDATE journal_entries SET body = ?2, word_count = ?3, edited_at = ?4, updated_at = ?5
         WHERE id = ?1",
        params![e.id, e.body, e.word_count, e.edited_at, e.updated_at],
    )?;
    Ok(())
}

/// Up to `limit` dates (newest first, strictly before `before` when given) with all their live
/// Entries, ordered newest date first and oldest Entry first within a date.
pub fn pages(
    conn: &Connection,
    before: Option<&str>,
    limit: u32,
) -> AppResult<Vec<(String, Vec<Entry>)>> {
    let mut dates = conn.prepare(
        "SELECT DISTINCT page_date FROM journal_entries
         WHERE deleted_at IS NULL AND (?1 IS NULL OR page_date < ?1)
         ORDER BY page_date DESC LIMIT ?2",
    )?;
    let dates: Vec<String> = dates
        .query_map(params![before, limit], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let (Some(newest), Some(oldest)) = (dates.first(), dates.last()) else {
        return Ok(Vec::new());
    };

    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM journal_entries
         WHERE deleted_at IS NULL AND page_date BETWEEN ?1 AND ?2
         ORDER BY page_date DESC, written_at ASC, id ASC"
    ))?;
    let entries = stmt
        .query_map(params![oldest, newest], from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    let mut out: Vec<(String, Vec<Entry>)> = Vec::with_capacity(dates.len());
    for entry in entries {
        match out.last_mut() {
            Some((date, group)) if *date == entry.page_date => group.push(entry),
            _ => out.push((entry.page_date.clone(), vec![entry])),
        }
    }
    Ok(out)
}

/// Sets the Page date only.
pub fn update_page_date(conn: &Connection, e: &Entry) -> AppResult<()> {
    conn.execute(
        "UPDATE journal_entries SET page_date = ?2, updated_at = ?3 WHERE id = ?1",
        params![e.id, e.page_date, e.updated_at],
    )?;
    Ok(())
}

/// Distinct dates with live Entries between `first` and `last` inclusive, oldest first.
pub fn dates_between(conn: &Connection, first: &str, last: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT page_date FROM journal_entries
         WHERE deleted_at IS NULL AND page_date BETWEEN ?1 AND ?2
         ORDER BY page_date",
    )?;
    Ok(stmt
        .query_map(params![first, last], |r| r.get(0))?
        .collect::<Result<_, _>>()?)
}

fn sql_int(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// What a search needs from an Entry.
pub struct SearchRow {
    pub id: String,
    pub page_date: String,
    pub body: String,
    pub written_at: i64,
    pub written_offset_min: i32,
    pub written_tz: String,
}

/// Live Entries whose Page date is within `from..=to` (each optional). With `matching`, only
/// those the FTS5 expression matches; with `excluding`, none that expression matches. Newest
/// Page first, oldest Entry first within a Page (the feed's order).
pub fn search(
    conn: &Connection,
    matching: Option<&str>,
    excluding: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
    limit: usize,
) -> AppResult<Vec<SearchRow>> {
    let mut sql = String::from(
        "SELECT e.id, e.page_date, e.body, e.written_at, e.written_offset_min, e.written_tz
         FROM journal_entries e
         WHERE e.deleted_at IS NULL
           AND (?3 IS NULL OR e.page_date >= ?3) AND (?4 IS NULL OR e.page_date <= ?4)",
    );
    if matching.is_some() {
        sql.push_str(" AND e.rowid IN (SELECT rowid FROM journal_fts WHERE journal_fts MATCH ?1)");
    }
    if excluding.is_some() {
        sql.push_str(
            " AND e.rowid NOT IN (SELECT rowid FROM journal_fts WHERE journal_fts MATCH ?2)",
        );
    }
    sql.push_str(" ORDER BY e.page_date DESC, e.written_at ASC, e.id ASC LIMIT ?5");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        params![matching, excluding, from, to, sql_int(limit)],
        |r| {
            Ok(SearchRow {
                id: r.get(0)?,
                page_date: r.get(1)?,
                body: r.get(2)?,
                written_at: r.get(3)?,
                written_offset_min: r.get(4)?,
                written_tz: r.get(5)?,
            })
        },
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Distinct indexed words of `min_len..=max_len` characters, with how many Entries hold each.
pub fn vocabulary(
    conn: &Connection,
    min_len: usize,
    max_len: usize,
) -> AppResult<Vec<(String, u32)>> {
    let mut stmt = conn
        .prepare("SELECT term, doc FROM journal_fts_vocab WHERE length(term) BETWEEN ?1 AND ?2")?;
    let rows = stmt.query_map(params![sql_int(min_len), sql_int(max_len)], |r| {
        Ok((r.get(0)?, r.get(1)?))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
