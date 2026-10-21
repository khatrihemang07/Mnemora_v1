//! SQL for Task search: Tasks whose title or notes match, or one of whose Comments does, and the
//! indexed vocabulary typo tolerance reads. No rules here; see `search.rs`.

use std::collections::HashMap;

use mnemora_core::error::AppResult;
use rusqlite::{Connection, params};

use crate::repo::{COLUMNS, corrupt_or_db, from_row};
use crate::task::Task;

fn sql_int(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Live Tasks the FTS5 expression `matching` finds in their title, notes or a live Comment, and
/// `excluding` (when given) does not. Open Tasks first in manual order, then the closed ones,
/// latest closed first.
pub fn search(
    conn: &Connection,
    matching: &str,
    excluding: Option<&str>,
    limit: usize,
) -> AppResult<Vec<Task>> {
    const HIT: &str = "(rowid IN (SELECT rowid FROM todo_tasks_fts WHERE todo_tasks_fts MATCH {})
         OR id IN (SELECT c.task_id FROM todo_comments c
                    WHERE c.deleted_at IS NULL AND c.rowid IN
                      (SELECT rowid FROM todo_comments_fts WHERE todo_comments_fts MATCH {})))";
    let mut sql = format!(
        "SELECT {COLUMNS} FROM todo_tasks WHERE deleted_at IS NULL AND {}",
        HIT.replace("{}", "?1")
    );
    if excluding.is_some() {
        sql.push_str(" AND NOT ");
        sql.push_str(&HIT.replace("{}", "?2"));
    }
    sql.push_str(
        " ORDER BY (state <> 'open'), CASE WHEN state = 'open' THEN sort_key END,
                   closed_at DESC, id LIMIT ?3",
    );
    let mut stmt = conn.prepare(&sql)?;
    stmt.query_map(params![matching, excluding, sql_int(limit)], from_row)?
        .collect::<Result<_, _>>()
        .map_err(corrupt_or_db)
}

/// Distinct indexed words (Task text and Comments) of `min_len..=max_len` characters, with how
/// many rows hold each.
pub fn vocabulary(
    conn: &Connection,
    min_len: usize,
    max_len: usize,
) -> AppResult<Vec<(String, u32)>> {
    let mut words: HashMap<String, u32> = HashMap::new();
    for table in ["todo_tasks_fts_vocab", "todo_comments_fts_vocab"] {
        let mut stmt = conn.prepare(&format!(
            "SELECT term, doc FROM {table} WHERE length(term) BETWEEN ?1 AND ?2"
        ))?;
        let rows = stmt.query_map(params![sql_int(min_len), sql_int(max_len)], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?))
        })?;
        for row in rows {
            let (term, docs) = row?;
            *words.entry(term).or_default() += docs;
        }
    }
    Ok(words.into_iter().collect())
}
