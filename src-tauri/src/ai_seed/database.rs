//! The Sandbox database as the seed finds it: whether it exists, is empty, holds the fixture, or
//! holds rows that are not the fixture's. Also the removal of its files, which only a reset does.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use mnemora_core::db::{self, Connection};

use super::err;
use super::marker::marker;

/// The tables a seed writes rows to. A database whose rows in these tables are not exactly what an
/// earlier seed wrote holds the user's own data.
const DATA_TABLES: [&str; 7] = [
    "todo_lists",
    "todo_tasks",
    "todo_comments",
    "journal_entries",
    "ai_conversations",
    "ai_entries",
    "ai_saved_questions",
];

/// What the seed found in the Sandbox database before it wrote anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbState {
    /// No database file yet.
    Missing,
    /// A database with no rows in the data tables.
    Empty,
    /// Holds exactly the rows an earlier seed wrote.
    Seeded,
    /// Holds rows that are not the fixture's.
    Other,
}

/// Looks at the database at `db_file` without changing it. Refuses (with a clear message) when another
/// connection is writing to it, since the seed would then compete with the app for the same rows.
pub fn inspect(db_file: &Path) -> Result<DbState, String> {
    if !db_file.is_file() {
        return Ok(DbState::Missing);
    }
    let Some(conn) = db::open_existing(db_file).map_err(err)? else {
        return Ok(DbState::Missing);
    };
    conn.busy_timeout(Duration::ZERO).map_err(err)?;
    if let Err(e) = conn.execute_batch("BEGIN IMMEDIATE; ROLLBACK;") {
        return Err(format!(
            "the Sandbox database is in use ({e}); quit the Sandbox app and try again"
        ));
    }
    let counts = count_rows(&conn)?;
    Ok(match marker(&conn)? {
        Some(recorded) if recorded == counts => DbState::Seeded,
        None if counts.values().all(|n| *n == 0) => DbState::Empty,
        _ => DbState::Other,
    })
}

/// Rows per data table; a table that does not exist yet counts as empty.
pub(super) fn count_rows(conn: &Connection) -> Result<BTreeMap<String, i64>, String> {
    let mut counts = BTreeMap::new();
    for table in DATA_TABLES {
        let n: i64 = if table_exists(conn, table)? {
            conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .map_err(err)?
        } else {
            0
        };
        counts.insert(table.to_owned(), n);
    }
    Ok(counts)
}

/// Whether a table of that name exists in the database.
pub(super) fn table_exists(conn: &Connection, name: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [name],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .map_err(err)
}

/// Deletes the database file and its WAL and shared-memory files. Only ever called with the Sandbox
/// path, after `refuse_production` has passed.
pub fn remove_database(db_file: &Path) -> Result<(), String> {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = db_file.as_os_str().to_owned();
        name.push(suffix);
        match fs::remove_file(&name) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!(
                    "could not remove {}: {e}",
                    Path::new(&name).display()
                ));
            }
        }
    }
    Ok(())
}
