//! Database backups using the `SQLite` online-backup API (safe while connections are open).

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::AppResult;
use crate::util::{compact_utc, now_ms};

/// How many backups are kept.
pub const KEEP_BACKUPS: usize = 5;

/// Writes a consistent copy of the database to `dir/<timestamp>-<from>-to-<to>.db`
/// and then prunes to the newest [`KEEP_BACKUPS`]. `at_ms` is injected for testability.
pub fn create_backup(
    conn: &Connection,
    dir: &Path,
    from: usize,
    to: usize,
    at_ms: i64,
) -> AppResult<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let name = format!("{}-{from}-to-{to}.db", compact_utc(at_ms));
    let dest = dir.join(&name);
    // Write beside the target and rename, so a crash never leaves a half file that looks valid.
    let partial = dir.join(format!("{name}.partial"));
    if let Err(e) = conn.backup(rusqlite::MAIN_DB, &partial, None) {
        let _ = std::fs::remove_file(&partial);
        return Err(e.into());
    }
    std::fs::rename(&partial, &dest)?;
    rotate(dir, KEEP_BACKUPS)?;
    Ok(dest)
}

/// A user-requested backup, named with the current total migration version on both sides.
pub fn backup_now(conn: &Connection, dir: &Path) -> AppResult<PathBuf> {
    let version: i64 = conn.query_row(
        "SELECT COALESCE(SUM(version), 0) FROM _module_migrations",
        [],
        |r| r.get(0),
    )?;
    let version = usize::try_from(version).unwrap_or(0);
    create_backup(conn, dir, version, version, now_ms())
}

/// Deletes all but the newest `keep` backups in `dir` (names sort chronologically).
/// Returns how many were removed.
pub fn rotate(dir: &Path, keep: usize) -> AppResult<usize> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.into()),
    };
    let mut backups: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "db"))
        .collect();
    // Timestamp prefixes sort chronologically, newest last.
    backups.sort();
    let excess = backups.len().saturating_sub(keep);
    for old in &backups[..excess] {
        std::fs::remove_file(old)?;
    }
    Ok(excess)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    fn seeded() -> Connection {
        let conn = crate::db::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t(x TEXT); INSERT INTO t VALUES ('kept');")
            .unwrap();
        conn
    }

    #[test]
    fn backup_is_named_and_restorable() {
        let dir = tempfile::tempdir().unwrap();
        let path = create_backup(&seeded(), &dir.path().join("backups"), 2, 5, 0).unwrap();
        assert_eq!(path.file_name().unwrap(), "19700101T000000000-2-to-5.db");
        let restored = Connection::open(&path).unwrap();
        let x: String = restored
            .query_row("SELECT x FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(x, "kept");
    }

    #[test]
    fn manual_backup_uses_current_version_on_both_sides() {
        let dir = tempfile::tempdir().unwrap();
        let conn = seeded();
        conn.execute_batch(
            "CREATE TABLE _module_migrations(module TEXT PRIMARY KEY, version INTEGER);
             INSERT INTO _module_migrations VALUES ('a', 2), ('b', 1);",
        )
        .unwrap();
        let path = backup_now(&conn, dir.path()).unwrap();
        assert!(path.to_string_lossy().ends_with("-3-to-3.db"), "{path:?}");
    }

    #[test]
    fn rotation_keeps_only_the_newest_five() {
        let dir = tempfile::tempdir().unwrap();
        let conn = seeded();
        for i in 0..7 {
            create_backup(
                &conn,
                dir.path(),
                i,
                i + 1,
                i64::try_from(i).unwrap() * 1000,
            )
            .unwrap();
        }
        let left = names(dir.path());
        assert_eq!(left.len(), KEEP_BACKUPS);
        // Oldest two (t=0s, t=1s) are gone.
        assert!(left[0].starts_with("19700101T000002"));
        assert!(left[4].starts_with("19700101T000006"));
    }

    #[test]
    fn rotate_ignores_unrelated_files_and_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(rotate(&dir.path().join("nope"), 5).unwrap(), 0);
        std::fs::write(dir.path().join("notes.txt"), "hi").unwrap();
        std::fs::write(dir.path().join("a.db"), "x").unwrap();
        std::fs::write(dir.path().join("b.db"), "x").unwrap();
        assert_eq!(rotate(dir.path(), 1).unwrap(), 1);
        assert_eq!(names(dir.path()), vec!["b.db", "notes.txt"]);
    }
}
