//! Tests for the Sandbox seed (`ai_seed.rs`): the Production refusals, the non-fixture refusal,
//! the database-in-use refusal, and that a seeded folder reads back through the change Tools with
//! the fixture's counts and seeds nothing twice. The expected values are the literals of
//! `ai_fixture_tests.rs`, which loads the same fixture on the same clock.

use mnemora_core::db::{self, Connection};
use mnemora_core::env::Environment;
use mnemora_core::module::Module;
use mnemora_core::paths::AppPaths;
use mnemora_core::tool::{Calendar, Tool, check};
use mnemora_journal::JournalModule;
use mnemora_todo::TodoModule;
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::ai_seed::{self, DbState, SeedOptions, SeedOutcome};

/// The device's zone, as in `ai_fixture_tests.rs`.
const ZONE: &str = "America/New_York";
/// 2026-10-25 12:00 in New York, as in `ai_fixture_tests.rs`. Yesterday is Saturday 24 October.
const NOW: i64 = 1_792_944_000_000;

/// A folder standing in for the platform's data folder. Production's folder would be inside it.
fn base() -> TempDir {
    tempfile::tempdir().unwrap()
}

/// Migrates the database at `paths` with every Module, as the app does at start.
fn migrated(paths: &AppPaths) -> Connection {
    let mut conn = db::open(&paths.db_file).unwrap();
    let modules = crate::modules();
    let refs: Vec<&dyn Module> = modules.iter().map(AsRef::as_ref).collect();
    db::migrate::run(&mut conn, &refs, &paths.backup_dir).unwrap();
    conn
}

/// Runs a change Tool on `conn` with the device in `ZONE`, and returns its body.
fn tool_body(module: &dyn Module, name: &str, args: &Value, conn: &Connection) -> Value {
    let tool: Box<dyn Tool> = module
        .tools()
        .into_iter()
        .find(|t| t.spec().name == name)
        .unwrap();
    let spec = tool.spec();
    let args = check(&spec, args).unwrap();
    let calendar = Calendar {
        today: "2026-10-25".to_owned(),
        time_zone: ZONE.to_owned(),
    };
    tool.run(conn, &args, &calendar).body
}

/// How many rows the tables that a repeated seed must not touch hold.
fn row_counts(conn: &Connection) -> Vec<i64> {
    ["todo_tasks", "journal_entries", "change_log"]
        .iter()
        .map(|t| {
            conn.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))
                .unwrap()
        })
        .collect()
}

#[test]
fn refuses_a_path_that_resolves_to_production() {
    let dir = base();
    let production = ai_seed::production_roots(dir.path());
    let targets = [
        AppPaths::resolve(Environment::Production, dir.path(), None),
        AppPaths::resolve(
            Environment::Sandbox,
            dir.path(),
            Some(&dir.path().join("com.hemang.mnemora/data")),
        ),
    ];
    for target in &targets {
        let err =
            ai_seed::seed_at(target, &production, NOW, ZONE, SeedOptions::default()).unwrap_err();
        assert!(err.contains("Production"), "{err}");
        assert!(!target.db_file.exists(), "a refused seed wrote a database");
    }
}

#[cfg(unix)]
#[test]
fn refuses_a_symlink_that_leads_into_production() {
    let dir = base();
    let production_data = dir.path().join("com.hemang.mnemora/data");
    std::fs::create_dir_all(&production_data).unwrap();
    let link = dir.path().join("sandbox-link");
    std::os::unix::fs::symlink(&production_data, &link).unwrap();
    let target = AppPaths::resolve(Environment::Sandbox, dir.path(), Some(&link));
    let err = ai_seed::seed_at(
        &target,
        &ai_seed::production_roots(dir.path()),
        NOW,
        ZONE,
        SeedOptions::default(),
    )
    .unwrap_err();
    assert!(err.contains("Production"), "{err}");
    assert!(!production_data.join("mnemora.db").exists());
}

#[test]
fn refuses_the_data_folder_that_production_has_moved_to() {
    let dir = base();
    let moved = dir.path().join("moved-production");
    let production = AppPaths::resolve(Environment::Production, dir.path(), None);
    let settings = mnemora_core::device::DeviceSettings {
        data_dir: Some(moved.to_string_lossy().into_owned()),
        ..Default::default()
    };
    settings.save(&production.device_file()).unwrap();
    let target = AppPaths::resolve(Environment::Sandbox, dir.path(), Some(&moved));
    let err = ai_seed::seed_at(
        &target,
        &ai_seed::production_roots(dir.path()),
        NOW,
        ZONE,
        SeedOptions::default(),
    )
    .unwrap_err();
    assert!(err.contains("Production"), "{err}");
}

#[test]
fn a_sandbox_folder_beside_production_is_allowed() {
    let dir = base();
    let target = AppPaths::resolve(Environment::Sandbox, dir.path(), None);
    let production = ai_seed::production_roots(dir.path());
    assert!(
        ai_seed::refuse_production(
            &[target.data_dir.as_path(), target.db_file.as_path()],
            &production,
        )
        .is_ok()
    );
    assert_eq!(ai_seed::inspect(&target.db_file), Ok(DbState::Missing));
}

#[test]
fn refuses_a_database_with_rows_that_are_not_the_fixture_until_reset() {
    let dir = base();
    let paths = AppPaths::resolve(Environment::Sandbox, dir.path(), None);
    let mut conn = migrated(&paths);
    mnemora_todo::create_list(&mut conn, "My own list").unwrap();
    drop(conn);
    let production = ai_seed::production_roots(dir.path());

    let err = ai_seed::seed_at(&paths, &production, NOW, ZONE, SeedOptions::default()).unwrap_err();
    assert!(err.contains("not the fixture"), "{err}");
    assert!(err.contains("--reset"), "{err}");
    let conn = db::open_existing(&paths.db_file).unwrap().unwrap();
    let lists: i64 = conn
        .query_row("SELECT count(*) FROM todo_lists", [], |r| r.get(0))
        .unwrap();
    assert_eq!(lists, 1, "a refused seed changed the database");
    drop(conn);

    let report = ai_seed::seed_at(
        &paths,
        &production,
        NOW,
        ZONE,
        SeedOptions {
            reset: true,
            ..SeedOptions::default()
        },
    )
    .unwrap();
    assert_eq!(report.before, DbState::Other);
    assert!(matches!(report.outcome, SeedOutcome::Written { .. }));
}

#[test]
fn refuses_a_database_another_connection_holds_open_for_writing() {
    let dir = base();
    let paths = AppPaths::resolve(Environment::Sandbox, dir.path(), None);
    drop(migrated(&paths));
    let holder = db::open_existing(&paths.db_file).unwrap().unwrap();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();

    let err = ai_seed::seed_at(
        &paths,
        &ai_seed::production_roots(dir.path()),
        NOW,
        ZONE,
        SeedOptions::default(),
    )
    .unwrap_err();
    assert!(err.contains("in use"), "{err}");
    holder.execute_batch("ROLLBACK").unwrap();
}

#[test]
fn seeding_reads_back_the_fixture_counts_through_the_change_tools_and_twice_changes_nothing() {
    let dir = base();
    let paths = AppPaths::resolve(Environment::Sandbox, dir.path(), None);
    let production = ai_seed::production_roots(dir.path());

    let first = ai_seed::seed_at(&paths, &production, NOW, ZONE, SeedOptions::default()).unwrap();
    assert_eq!(first.before, DbState::Missing);
    let SeedOutcome::Written {
        items,
        skipped,
        counts,
        ..
    } = first.outcome
    else {
        panic!("the first seed did not write: {first:?}");
    };
    assert_eq!(items, 112);
    assert_eq!(skipped, 0, "every fixture event is before the fixed now");
    // The fixture makes 3 Lists and 23 Journal Entries (its `list` and `entry` events).
    assert_eq!(counts["todo_lists"], 3, "{counts:?}");
    assert_eq!(counts["journal_entries"], 23, "{counts:?}");

    let conn = db::open_existing(&paths.db_file).unwrap().unwrap();
    let yesterday = json!({ "from": "2026-10-24", "to": "2026-10-24" });
    let todo = tool_body(&TodoModule, "todo_changes", &yesterday, &conn);
    assert_eq!(
        todo["counts"],
        json!({ "created": 4, "done": 3, "wont": 1, "skipped": 0, "reopened": 0, "moved": 1, "due": 2, "priority": 1, "edited": 0, "commented": 6, "deleted": 1, "restored": 0, "repeat": 0 }),
        "{todo}"
    );
    assert_eq!(todo["days"][0]["rows"].as_array().map(Vec::len), Some(19));
    let journal = tool_body(&JournalModule, "journal_changes", &yesterday, &conn);
    assert_eq!(
        journal["counts"],
        json!({ "written": 3, "edited": 1, "deleted": 1 }),
        "{journal}"
    );
    let before = row_counts(&conn);
    drop(conn);

    let again = ai_seed::seed_at(&paths, &production, NOW, ZONE, SeedOptions::default()).unwrap();
    assert_eq!(again.before, DbState::Seeded);
    assert_eq!(again.outcome, SeedOutcome::AlreadySeeded);
    let conn = db::open_existing(&paths.db_file).unwrap().unwrap();
    assert_eq!(row_counts(&conn), before, "a second seed wrote rows");
}

#[test]
fn a_seed_that_leaves_out_the_catch_up_day_is_read_back_from_its_database() {
    let full_dir = base();
    let full = AppPaths::resolve(Environment::Sandbox, full_dir.path(), None);
    let full_production = ai_seed::production_roots(full_dir.path());
    ai_seed::seed_at(&full, &full_production, NOW, ZONE, SeedOptions::default()).unwrap();
    assert_eq!(ai_seed::leaves_out_catchup(&full.db_file), Ok(false));

    let dir = base();
    let paths = AppPaths::resolve(Environment::Sandbox, dir.path(), None);
    let production = ai_seed::production_roots(dir.path());
    let report = ai_seed::seed_at(
        &paths,
        &production,
        NOW,
        ZONE,
        SeedOptions {
            skip_catchup: true,
            ..SeedOptions::default()
        },
    )
    .unwrap();
    let SeedOutcome::Written {
        items, left_out, ..
    } = report.outcome
    else {
        panic!("the seed did not write: {report:?}");
    };
    assert_eq!(items, 47);
    assert_eq!(left_out, 139);
    assert_eq!(ai_seed::leaves_out_catchup(&paths.db_file), Ok(true));
}
