//! Database backups using the `SQLite` online-backup API (safe while connections are open).

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::AppResult;
use crate::util::{compact_utc, now_ms};

/// How many backups are kept.
pub const KEEP_BACKUPS: usize = 5;

/// Marks the name of a backup taken before pairing with another device. These rotate on their
/// own, so frequent pairings never push out the backups taken before a migration (ADR 0004).
const PAIRING_MARK: &str = "-pairing-";

/// Writes a consistent copy of the database to `dir/<timestamp>-<from>-to-<to>.db`
/// and then prunes to the newest [`KEEP_BACKUPS`]. `at_ms` is injected for testability.
pub fn create_backup(
    conn: &Connection,
    dir: &Path,
    from: usize,
    to: usize,
    at_ms: i64,
) -> AppResult<PathBuf> {
    write_backup(
        conn,
        dir,
        &format!("{}-{from}-to-{to}.db", compact_utc(at_ms)),
    )
}

fn write_backup(conn: &Connection, dir: &Path, name: &str) -> AppResult<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let dest = dir.join(name);
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

/// A backup taken before this device merges with another one (pairing, or the first session
/// with a device it never met). Kept apart from the others: the newest [`KEEP_BACKUPS`] of each
/// kind stay.
pub fn backup_for_pairing(conn: &Connection, dir: &Path) -> AppResult<PathBuf> {
    let version: i64 = conn.query_row(
        "SELECT COALESCE(SUM(version), 0) FROM _module_migrations",
        [],
        |r| r.get(0),
    )?;
    write_backup(
        conn,
        dir,
        &format!("{}{PAIRING_MARK}{version}.db", compact_utc(now_ms())),
    )
}

/// Deletes all but the newest `keep` backups of each kind (migration and manual, pairing) in
/// `dir` (names sort chronologically). Returns how many were removed.
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
    let (pairing, other): (Vec<PathBuf>, Vec<PathBuf>) = backups.into_iter().partition(|p| {
        p.file_name()
            .is_some_and(|n| n.to_string_lossy().contains(PAIRING_MARK))
    });
    let mut removed = 0;
    for kind in [pairing, other] {
        let excess = kind.len().saturating_sub(keep);
        for old in &kind[..excess] {
            std::fs::remove_file(old)?;
        }
        removed += excess;
    }
    Ok(removed)
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
    fn pairing_backups_rotate_apart_from_migration_backups() {
        let dir = tempfile::tempdir().unwrap();
        let conn = seeded();
        conn.execute_batch(
            "CREATE TABLE _module_migrations(module TEXT PRIMARY KEY, version INTEGER);",
        )
        .unwrap();
        for i in 0..2 {
            create_backup(&conn, dir.path(), i, i + 1, i64::try_from(i).unwrap()).unwrap();
        }
        for _ in 0..8 {
            backup_for_pairing(&conn, dir.path()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let left = names(dir.path());
        assert_eq!(
            left.iter().filter(|n| n.contains("-pairing-")).count(),
            KEEP_BACKUPS
        );
        assert_eq!(
            left.iter().filter(|n| n.contains("-to-")).count(),
            2,
            "the migration backups are still there: {left:?}"
        );
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
