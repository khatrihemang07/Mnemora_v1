//! `SQLite` access: one file per Environment, opened with sane pragmas.

pub mod backup;
pub mod migrate;

use std::path::Path;
use std::sync::Arc;

pub use rusqlite::Connection;
use rusqlite::OpenFlags;
use rusqlite::functions::FunctionFlags;

use crate::error::AppResult;
use crate::util::now_ms;

/// Where a connection reads wall-clock milliseconds from. The system clock unless a test swaps
/// it (see [`set_time_source`]), so tests can simulate clock skew between devices.
pub type TimeSource = Arc<dyn Fn() -> i64 + Send + Sync>;

/// The SQL function that serves the connection's time source.
const TIME_FUNCTION: &str = "mnemora_now";

/// Replaces the wall clock this connection stamps change-log records with.
pub fn set_time_source(conn: &Connection, source: TimeSource) -> AppResult<()> {
    conn.create_scalar_function(TIME_FUNCTION, 0, FunctionFlags::SQLITE_UTF8, move |_| {
        Ok(source())
    })?;
    Ok(())
}

/// Wall-clock milliseconds as this connection's time source says; the system clock when none
/// was set.
pub fn physical_now(conn: &Connection) -> i64 {
    conn.query_row(&format!("SELECT {TIME_FUNCTION}()"), [], |r| r.get(0))
        .unwrap_or_else(|_| now_ms())
}

/// Opens (creating if needed) the database at `path` with WAL and foreign keys on.
pub fn open(path: &Path) -> AppResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    // journal_mode returns a row, so it cannot go through `execute`.
    conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
    configure(&conn)?;
    Ok(conn)
}

/// Opens the database at `path` only if it already exists; never creates the file or its folder.
/// `Ok(None)` means there is no database yet. For readers that must not make one (the Todo
/// widget): the app alone creates and migrates.
pub fn open_existing(path: &Path) -> AppResult<Option<Connection>> {
    if !path.is_file() {
        return Ok(None);
    }
    let conn =
        Connection::open_with_flags(path, OpenFlags::default() - OpenFlags::SQLITE_OPEN_CREATE)?;
    configure(&conn)?;
    Ok(Some(conn))
}

/// An in-memory database with the same pragmas that apply to memory databases; for tests.
pub fn open_in_memory() -> AppResult<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> AppResult<()> {
    conn.pragma_update(None, "foreign_keys", true)?;
    // Another connection (e.g. a backup) may briefly hold a lock.
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    set_time_source(conn, Arc::new(now_ms))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pragma_string(conn: &Connection, name: &str) -> String {
        conn.query_row(&format!("PRAGMA {name}"), [], |r| {
            r.get::<_, rusqlite::types::Value>(0)
        })
        .map(|v| match v {
            rusqlite::types::Value::Integer(i) => i.to_string(),
            rusqlite::types::Value::Text(t) => t,
            other => format!("{other:?}"),
        })
        .unwrap()
    }

    #[test]
    fn file_db_uses_wal_and_foreign_keys() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open(&dir.path().join("deep/er/x.db")).unwrap();
        assert_eq!(pragma_string(&conn, "journal_mode"), "wal");
        assert_eq!(pragma_string(&conn, "foreign_keys"), "1");
    }

    #[test]
    fn open_existing_returns_none_and_creates_nothing_for_a_missing_path() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("deep/er/x.db");
        assert!(open_existing(&missing).unwrap().is_none());
        assert!(!dir.path().join("deep").exists());
        assert!(!missing.exists());
    }

    #[test]
    fn open_existing_opens_a_database_made_by_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.db");
        open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE t(a); INSERT INTO t VALUES (7)")
            .unwrap();
        let conn = open_existing(&path).unwrap().unwrap();
        let a: i64 = conn.query_row("SELECT a FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(a, 7);
        assert_eq!(pragma_string(&conn, "foreign_keys"), "1");
    }

    #[test]
    fn in_memory_db_enforces_foreign_keys() {
        let conn = open_in_memory().unwrap();
        assert_eq!(pragma_string(&conn, "foreign_keys"), "1");
    }
}
