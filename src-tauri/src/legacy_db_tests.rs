//! Starting this release on a database the previous release left behind: every migration since
//! (core 3 onwards, Todo 5, all of Sync) runs on real-looking data, and the app's screens still
//! read it. The old schema is built by running each Module's own migrations only up to the
//! version the previous release had, so it cannot drift from the real history.

use mnemora_core::db::Connection;
use mnemora_core::device::DeviceSettings;
use mnemora_core::module::Module;
use mnemora_journal::JournalModule;
use mnemora_todo::TodoModule;
use serde_json::{Value, json};

use crate::test_support::TestApp;

/// Migrations each Module had in the previous release.
const OLD_CORE: usize = 2;
const OLD_JOURNAL: usize = 2;
const OLD_TODO: usize = 4;

const DAY: i64 = 86_400_000;
const T0: i64 = 1_700_000_000_000;

fn to_old_version(conn: &mut Connection, module: &dyn Module, version: usize) {
    module
        .migrations()
        .to_version(conn, version)
        .expect("old migrations");
    conn.pragma_update(None, "user_version", 0).unwrap();
    conn.execute_batch(&format!(
        "INSERT INTO _module_migrations(module, version) VALUES ('{}', {version})",
        module.id()
    ))
    .unwrap();
}

/// A year of use: Preferences (look and others), Lists, Tasks (one repeating with an old-style
/// occurrence, one deleted long ago), Comments (one edited), Entries and a change log with an
/// editing-session record.
fn previous_release(conn: &mut Connection) {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _module_migrations(
             module TEXT PRIMARY KEY NOT NULL, version INTEGER NOT NULL) STRICT",
    )
    .unwrap();
    to_old_version(conn, &mnemora_core::module::CoreModule, OLD_CORE);
    to_old_version(conn, &JournalModule, OLD_JOURNAL);
    to_old_version(conn, &TodoModule, OLD_TODO);
    let t = |n: i64| T0 + n * DAY;
    conn.execute_batch(&format!(
        "INSERT INTO settings(module, key, value, updated_at) VALUES
             ('core', 'theme', '\"dark\"', 0), ('core', 'phone_tabs', '[\"home\",\"todo\"]', 0),
             ('core', 'display_name', '\"Hemang\"', {t1});
         INSERT INTO todo_lists(id, name, name_key, emoji, sort_key, created_at, updated_at, deleted_at) VALUES
             ('l1', 'Groceries', 'groceries', NULL, 'a0', {t0}, {t0}, NULL),
             ('l2', 'Old list', 'old list', NULL, 'a1', {t0}, {t1}, {t1});
         INSERT INTO todo_tasks(id, list_id, title, notes, due_date, repeat_freq, repeat_every, repeat_mode,
                                priority, state, closed_at, repeat_of, sort_key, created_at, updated_at, deleted_at) VALUES
             ('t1', 'l1', 'Buy milk', 'two litres', '2026-01-05', 'week', 1, 'fixed', 2, 'open', NULL, NULL, 'a0', {t0}, {t0}, NULL),
             ('t1-old', 'l1', 'Buy milk', 'two litres', '2025-12-29', NULL, NULL, NULL, 2, 'done', {t1}, 't1', 'a1', {t0}, {t1}, NULL),
             ('t2', NULL, 'Call the bank', '', NULL, NULL, NULL, NULL, 4, 'open', NULL, NULL, 'a2', {t0}, {t0}, NULL),
             ('t3', 'l2', 'Gone', '', NULL, NULL, NULL, NULL, 4, 'open', NULL, NULL, 'a3', {t0}, {t1}, {t1});
         INSERT INTO todo_comments(id, task_id, body, created_at, updated_at, deleted_at) VALUES
             ('c1', 't1', 'plain', {t0}, {t0}, NULL), ('c2', 't1', 'edited later', {t0}, {t1}, NULL);
         INSERT INTO journal_entries(id, page_date, body, written_at, written_offset_min, written_tz,
                                     edited_at, word_count, created_at, updated_at, deleted_at) VALUES
             ('e1', '2026-01-02', 'A quiet day', {t0}, 60, 'Europe/Paris', NULL, 3, {t0}, {t0}, NULL),
             ('e2', '2026-01-03', 'Second entry', {t1}, 60, 'Europe/Paris', {t1}, 2, {t1}, {t1}, NULL);
         INSERT INTO change_log(id, module, entity, entity_id, op, session_id, snapshot, at) VALUES
             ('r1', 'todo', 'task', 't1', 'create', NULL, '{{\"title\":\"Buy milk\"}}', {t0}),
             ('r2', 'todo', 'task', 't1', 'update', 's1', '{{\"title\":\"Buy oat milk\"}}', {t1}),
             ('r3', 'journal', 'journal_entry', 'e1', 'create', NULL, '{{\"body\":\"A quiet day\"}}', {t0});",
        t0 = t(0),
        t1 = t(1),
    ))
    .unwrap();
}

fn app() -> TestApp {
    TestApp::legacy(previous_release, DeviceSettings::default())
}

#[test]
fn every_migration_runs_on_a_previous_release_database() {
    let app = app();
    app.with_db(|c| {
        let check: String = c
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(check, "ok");
        let broken: i64 = c
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(broken, 0, "foreign keys");
        let versions = mnemora_core::db::migrate::versions(c).unwrap();
        assert!(versions["core"] > OLD_CORE && versions["todo"] > OLD_TODO);
        assert!(versions["sync"] > 0);
    });
}

#[test]
fn the_change_log_keeps_its_rows_and_gets_a_stable_order() {
    let app = app();
    let device = app.device_id();
    app.with_db(|c| {
        let rows: Vec<(String, String, i64, i64, i64, i64)> = c
            .prepare("SELECT id, origin_device, hlc_ms, hlc_counter, seq, origin_seq FROM change_log ORDER BY seq")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let old: Vec<&(String, String, i64, i64, i64, i64)> =
            rows.iter().filter(|r| ["r1", "r2", "r3"].contains(&r.0.as_str())).collect();
        assert_eq!(old.len(), 3, "no record lost");
        for r in &old {
            assert_eq!(r.1, device, "stamped with this device");
            assert_eq!(r.3, 0);
            assert_eq!(r.4, r.5);
        }
        // History order: by time, then by insertion.
        let ids: Vec<&str> = old.iter().map(|r| r.0.as_str()).collect();
        assert_eq!(ids, ["r1", "r3", "r2"]);
        assert!(old.windows(2).all(|w| w[0].2 <= w[1].2));
        // The clock moves on from the newest old record, never behind it.
        let (ms, seq): (i64, i64) = c
            .query_row("SELECT ms, seq FROM device_clock", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert!(ms >= old.iter().map(|r| r.2).max().unwrap());
        assert!(seq >= rows.iter().map(|r| r.4).max().unwrap());
    });
}

#[test]
fn old_data_is_still_read_by_the_screens() {
    let app = app();
    let task: Value = app.invoke("todo_task", json!({ "id": "t1" })).unwrap();
    assert_eq!(task["title"], "Buy milk");
    assert_eq!(task["notes"], "two litres");
    let history: Value = app
        .invoke("todo_task_history", json!({ "id": "t1" }))
        .unwrap();
    assert!(history.is_array());
    let pages: Vec<Value> = app
        .invoke(
            "journal_pages",
            json!({ "cursor": null, "limit": 50, "direction": "older" }),
        )
        .unwrap();
    let entries: usize = pages
        .iter()
        .map(|p| p["entries"].as_array().unwrap().len())
        .sum();
    assert_eq!(entries, 2);
}

#[test]
fn comments_edited_before_the_upgrade_still_read_as_edited() {
    let app = app();
    let flags: Vec<(String, i64)> = app.with_db(|c| {
        c.prepare("SELECT id, edited FROM todo_comments ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    });
    assert_eq!(flags, [("c1".to_owned(), 0), ("c2".to_owned(), 1)]);
}

#[test]
fn look_preferences_move_to_the_device_file_and_others_stay() {
    let app = app();
    let values: Value = app.invoke("settings_get_all", json!({})).unwrap();
    assert_eq!(values["values"]["core.theme"], "dark");
    assert_eq!(values["values"]["core.display_name"], "Hemang");
    let left: i64 = app.with_db(|c| {
        c.query_row(
            "SELECT COUNT(*) FROM settings WHERE key IN ('theme','phone_tabs')",
            [],
            |r| r.get(0),
        )
        .unwrap()
    });
    assert_eq!(left, 0);
}

#[test]
fn nothing_syncs_and_nothing_is_stored_by_sync_until_it_is_turned_on() {
    let app = app();
    app.with_db(|c| {
        for table in [
            "sync_peers",
            "sync_conflicts",
            "sync_resolutions",
            "sync_activity",
            "sync_group",
        ] {
            let n: i64 = c
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 0, "{table}");
        }
    });
    let probe = app.sync_probe();
    assert!(
        !probe.running && probe.listeners == 0 && probe.advertisements == 0 && probe.tasks == 0
    );
}

#[test]
fn an_edit_after_the_upgrade_is_recorded_and_the_old_session_is_untouched() {
    let app = app();
    let _: Value = app
        .invoke(
            "todo_task_update",
            json!({ "id": "t2", "title": "Call the bank today", "notes": "", "sessionId": "after" }),
        )
        .unwrap();
    app.with_db(|c| {
        let n: i64 = c.query_row("SELECT COUNT(*) FROM change_log", [], |r| r.get(0)).unwrap();
        assert!(n >= 3);
        let dup: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM (SELECT seq FROM change_log GROUP BY seq HAVING COUNT(*) > 1)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dup, 0);
    });
}
