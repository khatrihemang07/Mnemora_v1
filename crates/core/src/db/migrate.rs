//! Runs every registered Module's migrations, each in its own namespace.
//!
//! `rusqlite_migration` tracks one counter in `PRAGMA user_version`. We give each Module
//! its own counter in `_module_migrations` and lend `user_version` to the library one
//! Module at a time, resetting it to 0 afterwards.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};
use rusqlite_migration::Migrations;

use super::backup;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::module::Module;
use crate::util::now_ms;

/// What a migration run did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Backup taken before upgrading existing data, if any.
    pub backup: Option<PathBuf>,
    /// `(module id, from version, to version)` for each Module that moved.
    pub applied: Vec<(String, usize, usize)>,
}

/// Migrates `conn` to the latest version of every Module. If existing data is about to be
/// upgraded, a backup goes to `backup_dir` first. Fresh databases are not backed up.
pub fn run(
    conn: &mut Connection,
    modules: &[&dyn Module],
    backup_dir: &Path,
) -> AppResult<MigrationReport> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _module_migrations(
             module TEXT PRIMARY KEY NOT NULL,
             version INTEGER NOT NULL
         ) STRICT",
    )?;
    let had_data: bool =
        conn.query_row("SELECT EXISTS(SELECT 1 FROM _module_migrations)", [], |r| {
            r.get(0)
        })?;

    let pending = plan(conn, modules)?;
    if pending.is_empty() {
        return Ok(MigrationReport::default());
    }

    let mut report = MigrationReport::default();
    if had_data {
        // Version numbers in the file name are sums across Modules: coarse, but enough to tell
        // backups apart and see how far each upgrade moved.
        let from = from_total(conn, modules)?;
        let to = from + pending.iter().map(|p| p.to - p.from).sum::<usize>();
        report.backup = Some(backup::create_backup(conn, backup_dir, from, to, now_ms())?);
    }

    for step in pending {
        let result = apply(conn, &step);
        // Always hand user_version back, even when the module failed.
        conn.pragma_update(None, "user_version", 0)?;
        result?;
        report
            .applied
            .push((step.id.to_owned(), step.from, step.to));
    }
    Ok(report)
}

struct Step {
    id: &'static str,
    from: usize,
    to: usize,
    migrations: Migrations<'static>,
}

fn stored_version(conn: &Connection, module: &str) -> AppResult<usize> {
    let v: Option<i64> = conn
        .query_row(
            "SELECT version FROM _module_migrations WHERE module = ?1",
            [module],
            |r| r.get(0),
        )
        .optional()?;
    Ok(v.map_or(0, |v| usize::try_from(v).unwrap_or(0)))
}

/// The version each Module's schema is at, by Module id (what the Sync handshake compares).
pub fn versions(conn: &Connection) -> AppResult<std::collections::BTreeMap<String, usize>> {
    let mut stmt = conn.prepare("SELECT module, version FROM _module_migrations")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    let mut out = std::collections::BTreeMap::new();
    for row in rows {
        let (module, version) = row?;
        out.insert(module, usize::try_from(version).unwrap_or(0));
    }
    Ok(out)
}

fn from_total(conn: &Connection, modules: &[&dyn Module]) -> AppResult<usize> {
    modules
        .iter()
        .try_fold(0, |acc, m| Ok(acc + stored_version(conn, m.id())?))
}

/// Whether every Module is at its latest version. Writes nothing (not even `user_version`, which
/// `plan` borrows), so a reader that must never migrate, such as the Todo widget, can ask.
/// A database without the migration table is behind. One newer than the app is an error.
pub fn is_current(conn: &Connection, modules: &[&dyn Module]) -> AppResult<bool> {
    let has_table: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_module_migrations')",
        [],
        |r| r.get(0),
    )?;
    if !has_table {
        return Ok(false);
    }
    // The version counter lives in the file header, so count pending steps on a scratch
    // in-memory connection instead of lending `user_version` on the real one.
    let scratch = Connection::open_in_memory()?;
    for module in modules {
        let from = stored_version(conn, module.id())?;
        scratch.pragma_update(None, "user_version", i64::try_from(from).unwrap_or(0))?;
        let pending = module.migrations().pending_migrations(&scratch)?;
        if pending < 0 {
            return Err(AppError::new(
                ErrorCode::Database,
                "The database is newer than this version of the app",
            )
            .with_details(format!("module {} is at version {from}", module.id())));
        }
        if pending > 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Works out which Modules have pending migrations, without changing any schema.
fn plan(conn: &Connection, modules: &[&dyn Module]) -> AppResult<Vec<Step>> {
    let mut steps = Vec::new();
    for module in modules {
        let from = stored_version(conn, module.id())?;
        let migrations = module.migrations();
        conn.pragma_update(None, "user_version", i64::try_from(from).unwrap_or(0))?;
        let pending = migrations.pending_migrations(conn);
        conn.pragma_update(None, "user_version", 0)?;
        let pending = pending?;
        if pending < 0 {
            return Err(AppError::new(
                ErrorCode::Database,
                "The database is newer than this version of the app",
            )
            .with_details(format!("module {} is at version {from}", module.id())));
        }
        if pending > 0 {
            let to = from + usize::try_from(pending).unwrap_or(0);
            steps.push(Step {
                id: module.id(),
                from,
                to,
                migrations,
            });
        }
    }
    Ok(steps)
}

fn apply(conn: &mut Connection, step: &Step) -> AppResult<()> {
    conn.pragma_update(None, "user_version", i64::try_from(step.from).unwrap_or(0))?;
    step.migrations.to_latest(conn)?;
    conn.execute(
        "INSERT INTO _module_migrations(module, version) VALUES (?1, ?2)
         ON CONFLICT(module) DO UPDATE SET version = excluded.version",
        rusqlite::params![step.id, i64::try_from(step.to).unwrap_or(0)],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite_migration::{M, Migrations};

    use super::*;
    use crate::settings::SettingsRegistry;

    struct Fake {
        id: &'static str,
        sql: &'static [&'static str],
    }

    impl Module for Fake {
        fn id(&self) -> &'static str {
            self.id
        }
        fn migrations(&self) -> Migrations<'static> {
            Migrations::new(self.sql.iter().map(|s| M::up(s)).collect())
        }
        fn register_settings(&self, _: &mut SettingsRegistry) {}
    }

    const A: Fake = Fake {
        id: "a",
        sql: &[
            "CREATE TABLE a_items(id INTEGER)",
            "ALTER TABLE a_items ADD COLUMN name TEXT",
        ],
    };
    const B: Fake = Fake {
        id: "b",
        sql: &["CREATE TABLE b_items(id INTEGER)"],
    };

    fn version(conn: &Connection, module: &str) -> Option<i64> {
        conn.query_row(
            "SELECT version FROM _module_migrations WHERE module = ?1",
            [module],
            |r| r.get(0),
        )
        .ok()
    }

    fn has_table(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE name = ?1",
            [name],
            |_| Ok(()),
        )
        .is_ok()
    }

    #[test]
    fn modules_migrate_independently_and_record_versions() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open_in_memory().unwrap();
        let report = run(&mut conn, &[&A, &B], dir.path()).unwrap();
        assert!(has_table(&conn, "a_items") && has_table(&conn, "b_items"));
        assert_eq!(version(&conn, "a"), Some(2));
        assert_eq!(version(&conn, "b"), Some(1));
        assert_eq!(report.applied, vec![("a".into(), 0, 2), ("b".into(), 0, 1)]);
        let uv: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, 0, "user_version is only borrowed");
    }

    #[test]
    fn rerun_is_a_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open_in_memory().unwrap();
        run(&mut conn, &[&A], dir.path()).unwrap();
        let again = run(&mut conn, &[&A], dir.path()).unwrap();
        assert_eq!(again, MigrationReport::default());
    }

    #[test]
    fn fresh_database_is_not_backed_up() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open_in_memory().unwrap();
        let report = run(&mut conn, &[&A], dir.path()).unwrap();
        assert_eq!(report.backup, None);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn upgrading_existing_data_backs_up_first() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open_in_memory().unwrap();
        run(&mut conn, &[&A], dir.path()).unwrap();
        conn.execute("INSERT INTO a_items(id, name) VALUES (1, 'x')", [])
            .unwrap();

        // B is new: existing data, pending migration, so a backup must appear.
        let report = run(&mut conn, &[&A, &B], dir.path()).unwrap();
        let backup = report.backup.expect("backup expected");
        let name = backup.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.ends_with("-2-to-3.db"), "{name}");
        let copy = Connection::open(&backup).unwrap();
        let n: i64 = copy
            .query_row("SELECT COUNT(*) FROM a_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        assert!(
            !has_table(&copy, "b_items"),
            "backup predates the migration"
        );
    }

    #[test]
    fn is_current_reports_behind_then_current_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open_in_memory().unwrap();
        let one_short = Fake {
            id: "a",
            sql: &["CREATE TABLE a_items(id INTEGER)"],
        };
        assert!(!is_current(&conn, &[&A]).unwrap(), "no migration table yet");

        run(&mut conn, &[&one_short], dir.path()).unwrap();
        assert!(is_current(&conn, &[&one_short]).unwrap());
        assert!(!is_current(&conn, &[&A]).unwrap(), "one migration short");
        assert!(!is_current(&conn, &[&one_short, &B]).unwrap(), "new Module");
        let uv: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(uv, 0, "the check leaves user_version alone");
        assert_eq!(version(&conn, "a"), Some(1), "and migrates nothing");

        run(&mut conn, &[&A, &B], dir.path()).unwrap();
        assert!(is_current(&conn, &[&A, &B]).unwrap());
    }

    #[test]
    fn is_current_refuses_a_database_newer_than_the_app() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open_in_memory().unwrap();
        run(&mut conn, &[&A], dir.path()).unwrap();
        let old_a = Fake {
            id: "a",
            sql: &["CREATE TABLE a_items(id INTEGER)"],
        };
        let err = is_current(&conn, &[&old_a]).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::Database);
    }

    #[test]
    fn database_newer_than_the_app_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = crate::db::open_in_memory().unwrap();
        run(&mut conn, &[&A], dir.path()).unwrap();
        let old_a = Fake {
            id: "a",
            sql: &["CREATE TABLE a_items(id INTEGER)"],
        };
        let err = run(&mut conn, &[&old_a], dir.path()).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::Database);
    }
}
