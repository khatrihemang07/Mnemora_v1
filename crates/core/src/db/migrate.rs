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

fn from_total(conn: &Connection, modules: &[&dyn Module]) -> AppResult<usize> {
    modules
        .iter()
        .try_fold(0, |acc, m| Ok(acc + stored_version(conn, m.id())?))
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
