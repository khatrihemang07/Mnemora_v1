//! The first merge combines everything both devices hold, whatever the change log remembers of it
//! (DV1, DV10): rows inserted around the log, rows from older versions whose records lack fields,
//! Preferences set before they were logged, deleted rows, occurrences. Every device is a `TestApp`
//! on a [`Cluster`]; the rows are written with raw SQL because that is exactly what a log without
//! history looks like.

use mnemora_core::db::Connection as Db;
use mnemora_core::device::DeviceSettings;
use mnemora_core::syncable::Holdings;
use mnemora_sync::PairingHost;
use serde_json::{Value, json};

use crate::sync::AppHost;
use crate::sync_lists_tests::world;
use crate::test_support::{Cluster, TestApp};

/// A device with Sync on and `seed` written to its freshly migrated database, not to the log.
fn device(seed: impl FnOnce(&Db)) -> TestApp {
    let mut settings = DeviceSettings::default();
    settings.sync.enabled = true;
    TestApp::upgrading(seed, settings)
}

/// `NULL` or the number, for SQL written by hand in these tests.
fn n(v: Option<i64>) -> String {
    v.map_or_else(|| "NULL".to_owned(), |v| v.to_string())
}

/// A quoted SQL string, or `NULL`.
fn q(v: Option<&str>) -> String {
    v.map_or_else(
        || "NULL".to_owned(),
        |v| format!("'{}'", v.replace('\'', "''")),
    )
}

fn list(db: &Db, id: &str, name: &str, deleted: Option<i64>, at: i64) {
    db.execute_batch(&format!(
        "INSERT INTO todo_lists(id, name, name_key, emoji, sort_key, created_at, updated_at, deleted_at)
         VALUES ('{id}', {name}, lower({name}), '📋', '{id}', {at}, {at}, {deleted})",
        name = q(Some(name)),
        deleted = n(deleted),
    ))
    .unwrap();
}

fn task(
    db: &Db,
    id: &str,
    title: &str,
    list: Option<&str>,
    repeat_of: Option<&str>,
    deleted: Option<i64>,
    at: i64,
) {
    let occurrence = repeat_of.is_some();
    db.execute_batch(&format!(
        "INSERT INTO todo_tasks(id, list_id, title, notes, due_date, repeat_freq, repeat_every,
                                repeat_mode, priority, state, closed_at, repeat_of, sort_key,
                                created_at, updated_at, deleted_at)
         VALUES ('{id}', {list}, {title}, 'some notes', '2026-10-30', {freq}, {every}, {mode}, 2,
                 '{state}', {closed}, {parent}, '{id}', {at}, {at}, {deleted})",
        list = q(list),
        title = q(Some(title)),
        freq = if occurrence { "NULL" } else { "'day'" },
        every = if occurrence { "NULL" } else { "1" },
        mode = if occurrence { "NULL" } else { "'fixed'" },
        state = if occurrence { "done" } else { "open" },
        closed = if occurrence {
            at.to_string()
        } else {
            "NULL".into()
        },
        parent = q(repeat_of),
        deleted = n(deleted),
    ))
    .unwrap();
}

fn comment(db: &Db, id: &str, task: &str, body: &str, deleted: Option<i64>, at: i64) {
    db.execute_batch(&format!(
        "INSERT INTO todo_comments(id, task_id, body, edited, created_at, updated_at, deleted_at)
         VALUES ('{id}', '{task}', {body}, {edited}, {at}, {at}, {deleted})",
        body = q(Some(body)),
        edited = i32::from(deleted.is_some()),
        deleted = n(deleted),
    ))
    .unwrap();
}

fn entry(db: &Db, id: &str, body: &str, deleted: Option<i64>, at: i64) {
    db.execute_batch(&format!(
        "INSERT INTO journal_entries(id, page_date, body, written_at, written_offset_min,
                                     written_tz, word_count, created_at, updated_at, deleted_at)
         VALUES ('{id}', '2026-04-01', {body}, {at}, 330, 'Asia/Kolkata', {words}, {at}, {at}, {deleted})",
        body = q(Some(body)),
        words = body.split_whitespace().count(),
        deleted = n(deleted),
    ))
    .unwrap();
}

fn pref(db: &Db, module: &str, key: &str, value: &str, at: i64) {
    db.execute_batch(&format!(
        "INSERT INTO settings(module, key, value, updated_at)
         VALUES ('{module}', '{key}', {value}, {at})",
        value = q(Some(value)),
    ))
    .unwrap();
}

/// What the change log holds for rows written by this device.
fn own_records(app: &TestApp) -> i64 {
    app.with_db(|db| {
        db.query_row(
            "SELECT COUNT(*) FROM change_log WHERE module != 'sync' AND origin_device = (SELECT device_id FROM device_clock)",
            [],
            |r| r.get(0),
        )
        .unwrap()
    })
}

fn holdings(app: &TestApp) -> Holdings {
    AppHost::new(app.handle(), false)
        .unwrap()
        .holdings()
        .unwrap()
}

const T0: i64 = Cluster::START - 10_000_000;

fn mac(db: &Db) {
    list(db, "l-home", "Home", None, T0);
    list(db, "l-old", "Old", Some(T0 + 5), T0 + 1);
    task(db, "t-inbox", "Inbox thing", None, None, None, T0 + 10);
    task(db, "t-home", "Hoover", Some("l-home"), None, None, T0 + 20);
    // A repeating Task's occurrence, done, copy of a live Task; and a deleted Task in a List.
    task(
        db,
        "t-occ",
        "Hoover",
        Some("l-home"),
        Some("t-home"),
        None,
        T0 + 30,
    );
    task(
        db,
        "t-gone",
        "Binned",
        Some("l-old"),
        None,
        Some(T0 + 40),
        T0 + 35,
    );
    comment(db, "c-live", "t-home", "mind the corners", None, T0 + 50);
    comment(db, "c-gone", "t-home", "never mind", Some(T0 + 60), T0 + 55);
    entry(db, "e-live", "Dear diary", None, T0 + 70);
    entry(db, "e-gone", "Oops", Some(T0 + 80), T0 + 75);
    pref(db, "core", "time_format", "\"12h\"", T0 + 90);
    pref(db, "core", "week_start", "\"sun\"", T0 + 91);
    // Look and feel is per device and stays home.
    pref(db, "core", "phone_tabs_edited", "true", T0 + 92);
}

fn phone(db: &Db) {
    list(db, "l-groc", "Groceries", None, T0 + 2);
    task(db, "p-milk", "Milk", Some("l-groc"), None, None, T0 + 12);
    task(db, "p-bin", "Phone bin", None, None, Some(T0 + 45), T0 + 41);
    comment(db, "pc-1", "p-milk", "oat", None, T0 + 52);
    entry(db, "pe-1", "Phone thought", None, T0 + 72);
    // Newer than the Mac's, so it wins.
    pref(db, "core", "week_start", "\"mon\"", T0 + 95);
}

fn every_row(app: &TestApp) -> Value {
    world(app)
}

#[test]
fn rows_without_any_log_history_reach_the_other_device_at_the_first_sync() {
    let c = Cluster::from_devices(vec![device(mac), device(phone)]);
    assert_eq!(
        own_records(&c[0]),
        0,
        "nothing is logged: this is the bug's setup"
    );

    // The preview counts what each device holds; the merge sends the same.
    let (mac_has, phone_has) = (holdings(&c[0]), holdings(&c[1]));
    assert_eq!(
        (mac_has.entries, mac_has.tasks, mac_has.lists.len()),
        (1, 3, 1)
    );
    assert_eq!(
        (phone_has.entries, phone_has.tasks, phone_has.lists.len()),
        (1, 1, 1)
    );

    c.sync(0, 1);
    c.sync(0, 1);

    let (on_mac, on_phone) = (every_row(&c[0]), every_row(&c[1]));
    assert_eq!(on_mac, on_phone, "both devices hold the same rows");
    // Everything both held, deleted rows included, is on both.
    assert_eq!(on_mac["lists"].as_array().unwrap().len(), 3);
    assert_eq!(on_mac["tasks"].as_array().unwrap().len(), 6);
    assert_eq!(on_mac["comments"].as_array().unwrap().len(), 3);
    assert_eq!(on_mac["entries"].as_array().unwrap().len(), 3);
    // The merged preview after the merge is the sum of the two, nothing went missing.
    let after = holdings(&c[0]);
    assert_eq!(after, holdings(&c[1]));
    assert_eq!(after.tasks, mac_has.tasks + phone_has.tasks);
    assert_eq!(after.entries, mac_has.entries + phone_has.entries);
    assert_eq!(
        after.lists.len(),
        mac_has.lists.len() + phone_has.lists.len()
    );
    // Preferences: the newest per key wins; the older Mac's "12h" had no rival.
    let pref_of = |app: &TestApp, key: &str| -> Value {
        let all: Value = app.invoke("settings_get_all", json!({})).unwrap();
        all["values"][key].clone()
    };
    for d in 0..2 {
        assert_eq!(pref_of(&c[d], "core.time_format"), "12h", "device {d}");
        assert_eq!(pref_of(&c[d], "core.week_start"), "mon", "device {d}");
    }
    // The deleted rows stay deleted, with their Comments.
    let gone: i64 = c[1].with_db(|db| {
        db.query_row(
            "SELECT COUNT(*) FROM todo_tasks WHERE id = 't-gone' AND deleted_at IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap()
    });
    assert_eq!(gone, 1);
}

#[test]
fn a_second_start_or_sync_adds_nothing_and_logged_rows_get_no_synthetic_records() {
    let c = Cluster::from_devices(vec![device(mac), device(phone)]);
    c.sync(0, 1);
    let (mac_rows, phone_rows) = (own_records(&c[0]), own_records(&c[1]));
    assert!(mac_rows > 0 && phone_rows > 0, "the rows joined the log");
    c.sync(0, 1);
    c.sync(1, 0);
    assert_eq!(
        (own_records(&c[0]), own_records(&c[1])),
        (mac_rows, phone_rows)
    );

    // A device whose rows were all made through the app logs nothing extra.
    let c = Cluster::new(2);
    let work: Value = c[0]
        .invoke("todo_list_create", json!({ "name": "Work" }))
        .unwrap();
    let report: Value = c[0]
        .invoke(
            "todo_task_create",
            json!({ "title": "Report", "listId": work["id"] }),
        )
        .unwrap();
    c[0].invoke::<Value>(
        "todo_comment_add",
        json!({ "taskId": report["id"], "body": "hi" }),
    )
    .unwrap();
    c[0].invoke::<Value>(
        "todo_task_set_priority",
        json!({ "id": report["id"], "priority": 1 }),
    )
    .unwrap();
    let before = own_records(&c[0]);
    c.sync(0, 1);
    assert_eq!(
        own_records(&c[0]),
        before,
        "no synthetic record for logged rows"
    );
}

#[test]
fn a_row_whose_log_is_partial_sends_what_the_log_lacks() {
    // An older version logged the Task's creation but not later priority, title or List changes.
    let c = Cluster::from_devices(vec![device(|_| {}), device(|_| {})]);
    let work: Value = c[0]
        .invoke("todo_list_create", json!({ "name": "Work" }))
        .unwrap();
    let report: Value = c[0]
        .invoke("todo_task_create", json!({ "title": "Report" }))
        .unwrap();
    c[0].with_db(|db| {
        db.execute(
            &format!(
                "UPDATE todo_tasks SET title = 'Report v2', priority = 1, list_id = '{}', updated_at = {}",
                work["id"].as_str().unwrap(),
                Cluster::START + 5
            ),
            [],
        )
        .unwrap();
        db.execute("UPDATE todo_lists SET name = 'Work!', name_key = 'work!'", [])
            .unwrap();
    });
    // Fresh runtime: the partial log has not been looked at yet.
    c.sync(0, 1);
    let got: Value = c[1]
        .invoke("todo_task", json!({ "id": report["id"] }))
        .unwrap();
    assert_eq!(got["title"], "Report v2");
    assert_eq!(got["priority"], 1);
    assert_eq!(got["list_id"], work["id"]);
    assert_eq!(every_row(&c[0]), every_row(&c[1]));
}

#[test]
fn a_synthetic_record_does_not_beat_a_real_later_edit() {
    let c = Cluster::from_devices(vec![
        device(|db| task(db, "t-1", "Old title", None, None, None, T0)),
        device(|_| {}),
    ]);
    c.sync(0, 1);
    // A real edit on the phone, later than the row's own time, comes back to the Mac.
    c[1].clock().advance(10);
    c[1].invoke::<Value>(
        "todo_task_set_priority",
        json!({ "id": "t-1", "priority": 1 }),
    )
    .unwrap();
    c.sync(0, 1);
    for d in 0..2 {
        let got: Value = c[d].invoke("todo_task", json!({ "id": "t-1" })).unwrap();
        assert_eq!(got["priority"], 1, "device {d}");
        assert_eq!(got["title"], "Old title", "device {d}");
    }
}

#[test]
fn an_older_unlogged_preference_loses_to_a_newer_logged_one() {
    let c = Cluster::from_devices(vec![
        device(|db| pref(db, "core", "time_format", "\"12h\"", T0)),
        device(|_| {}),
    ]);
    c[1].set_pref("core.time_format", json!("24h"));
    c.sync(0, 1);
    for d in 0..2 {
        let all: Value = c[d].invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(all["values"]["core.time_format"], "24h", "device {d}");
    }
}

/// Gives every device of `c` rows the log never heard of (for the convergence run): a List, Tasks
/// in it and in the Inbox (one deleted, one an occurrence), a Comment, an Entry, and a Preference
/// every device has an opinion on. Call before the devices first sync.
pub(crate) fn seed_unlogged(c: &Cluster, devices: usize) {
    for d in 0..devices {
        c[d].with_db(|db| {
            let (home, task_id, occ, gone, note, diary) = (
                format!("u{d}-list"),
                format!("u{d}-task"),
                format!("u{d}-occ"),
                format!("u{d}-gone"),
                format!("u{d}-comment"),
                format!("u{d}-entry"),
            );
            let at = T0 + i64::try_from(d).unwrap() * 1000;
            list(db, &home, &format!("Unlogged {d}"), None, at);
            task(
                db,
                &task_id,
                &format!("Unlogged task {d}"),
                Some(&home),
                None,
                None,
                at + 1,
            );
            task(
                db,
                &occ,
                &format!("Unlogged task {d}"),
                Some(&home),
                Some(&task_id),
                None,
                at + 2,
            );
            task(
                db,
                &gone,
                "Unlogged deleted",
                None,
                None,
                Some(at + 4),
                at + 3,
            );
            comment(db, &note, &task_id, "unlogged comment", None, at + 5);
            entry(db, &diary, "unlogged entry", None, at + 6);
            pref(
                db,
                "core",
                "week_start",
                if d % 2 == 0 { "\"sun\"" } else { "\"mon\"" },
                at + 7,
            );
        });
    }
}
