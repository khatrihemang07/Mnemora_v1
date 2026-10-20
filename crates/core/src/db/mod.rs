//! `SQLite` access: one file per Environment, opened with sane pragmas.

pub mod backup;
pub mod migrate;

use std::path::Path;

pub use rusqlite::Connection;

use crate::error::AppResult;

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
    fn in_memory_db_enforces_foreign_keys() {
        let conn = open_in_memory().unwrap();
        assert_eq!(pragma_string(&conn, "foreign_keys"), "1");
    }
}
