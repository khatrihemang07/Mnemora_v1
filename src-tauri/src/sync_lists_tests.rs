//! Sync scenarios for Lists, Comments and Preferences on a [`Cluster`] (ADR 0017): the List name
//! clash, a Task moved into a List deleted elsewhere, a Comment on a deleted Task, and
//! Preferences that sync while look and feel stays per device. Every step is a real command; the
//! assertions read back through commands, except the "world" dumps that compare whole devices.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use crate::test_support::{Cluster, TestApp};

const TODAY: &str = "2026-10-22";

fn local() -> Value {
    json!({ "today": TODAY, "now": "14:05", "offset_min": 0 })
}

fn create_list(app: &TestApp, name: &str) -> Value {
    // List ids are UUIDv7 from the real clock: a pause makes "older" unambiguous.
    std::thread::sleep(Duration::from_millis(3));
    app.invoke("todo_list_create", json!({ "name": name }))
        .unwrap()
}

fn rename_list(app: &TestApp, id: &Value, name: &str) {
    app.invoke::<Value>("todo_list_rename", json!({ "id": id, "name": name }))
        .unwrap();
}

fn task_in(app: &TestApp, title: &str, list: Option<&Value>) -> Value {
    app.invoke(
        "todo_task_create",
        json!({ "title": title, "listId": list }),
    )
    .unwrap()
}

fn task(app: &TestApp, id: &Value) -> Value {
    app.invoke("todo_task", json!({ "id": id })).unwrap()
}

/// The live Lists in sidebar order: `(id, name)`.
fn lists(app: &TestApp) -> Vec<(String, String)> {
    let view: Value = app
        .invoke("todo_lists", json!({ "local": local() }))
        .unwrap();
    view["lists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["list"]["id"].as_str().unwrap().to_owned(),
                e["list"]["name"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// Titles of the open Tasks on a List's page, in the order the page shows them.
fn titles_in(app: &TestApp, list: &Value) -> Vec<String> {
    let view: Value = app
        .invoke("todo_list", json!({ "id": list, "local": local() }))
        .unwrap();
    view["groups"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|g| g["tasks"].as_array().unwrap().iter())
        .map(|t| t["title"].as_str().unwrap().to_owned())
        .collect()
}

fn list_gone(app: &TestApp, list: &Value) -> bool {
    app.invoke::<Value>("todo_list", json!({ "id": list, "local": local() }))
        .is_err_and(|e| e["code"] == "not_found")
}

fn inbox_titles(app: &TestApp) -> Vec<String> {
    let view: Value = app
        .invoke("todo_inbox", json!({ "local": local() }))
        .unwrap();
    let mut titles: Vec<String> = crate::test_support::inbox_tasks_of(&view)
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_owned())
        .collect();
    titles.sort();
    titles
}

/// Everything about Lists, Tasks' Lists and order, Comments and Preferences on one device, as
/// plain data: two devices that agree on this agree on what the user sees.
pub(crate) fn world(app: &TestApp) -> Value {
    app.with_db(|db| {
        let rows = |columns: &str, table: &str, order: &str| -> Vec<Value> {
            let sql = format!("SELECT json_array({columns}) FROM {table} ORDER BY {order}");
            let mut stmt = db.prepare(&sql).unwrap();
            stmt.query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .map(|row| serde_json::from_str(&row.unwrap()).unwrap())
                .collect()
        };
        json!({
            "lists": rows("id, name, emoji, sort_key, task_sort, group_by, deleted_at IS NOT NULL", "todo_lists", "id"),
            // The List of a deleted Task is hidden state that depends on whether its List or the
            // Task went first; Restore settles it the same way everywhere.
            "tasks": rows("id, CASE WHEN deleted_at IS NULL THEN list_id END, sort_key, title, deleted_at IS NOT NULL", "todo_tasks", "id"),
            "comments": rows("id, task_id, body, edited, deleted_at IS NOT NULL", "todo_comments", "id"),
            "settings": rows("module, key, value", "settings", "module, key"),
            "entries": rows("id, page_date, body, written_at, written_offset_min, written_tz, edited_at, word_count, created_at, deleted_at IS NOT NULL", "journal_entries", "id"),
        })
    })
}

fn same_world(c: &Cluster, devices: usize) {
    let first = world(&c[0]);
    for d in 1..devices {
        assert_eq!(world(&c[d]), first, "device {d} differs from device 0");
    }
}

// ---------------------------------------------------------------- the name clash

/// Two devices each make a "Groceries" with Tasks; returns (cluster, older List, newer List).
fn clash_on_creation() -> (Cluster, Value, Value) {
    let c = Cluster::new(2);
    let older = create_list(&c[0], "Groceries");
    c[0].invoke::<Value>(
        "todo_list_set_emoji",
        json!({ "id": older["id"], "emoji": "🛒" }),
    )
    .unwrap();
    c[0].invoke::<Value>(
        "todo_list_set_sort",
        json!({ "id": older["id"], "sort": "title" }),
    )
    .unwrap();
    for title in ["Milk", "Eggs"] {
        task_in(&c[0], title, Some(&older["id"]));
    }
    // Accents and case are ignored when names are compared.
    let newer = create_list(&c[1], "GROCERIES");
    c[1].invoke::<Value>(
        "todo_list_set_emoji",
        json!({ "id": newer["id"], "emoji": "🥕" }),
    )
    .unwrap();
    for title in ["Bread", "Apples"] {
        task_in(&c[1], title, Some(&newer["id"]));
    }
    assert!(older["id"].as_str() < newer["id"].as_str());
    (c, older["id"].clone(), newer["id"].clone())
}

#[test]
fn same_name_lists_made_on_two_devices_merge_into_the_older_which_keeps_everything() {
    let (c, older, newer) = clash_on_creation();
    c.sync(0, 1);
    c.sync(0, 1);
    for d in 0..2 {
        let all = lists(&c[d]);
        assert_eq!(
            all,
            vec![(older.as_str().unwrap().into(), "Groceries".into())],
            "device {d}"
        );
        let kept: Value = c[d]
            .invoke("todo_list", json!({ "id": older, "local": local() }))
            .unwrap();
        assert_eq!(kept["list"]["emoji"], "🛒", "the older keeps its emoji");
        assert_eq!(kept["list"]["task_sort"], "title", "and its sort");
        assert!(list_gone(&c[d], &newer), "the newer is gone on device {d}");
    }
    same_world(&c, 2);
}

#[test]
fn the_newer_lists_tasks_join_the_end_of_the_older_ones_manual_order_in_their_own_order() {
    let (c, older, _) = clash_on_creation();
    // Manual order, so the page shows the stored order.
    c[0].invoke::<Value>(
        "todo_list_set_sort",
        json!({ "id": older, "sort": "manual" }),
    )
    .unwrap();
    c.sync(0, 1);
    c.sync(0, 1);
    for d in 0..2 {
        assert_eq!(
            titles_in(&c[d], &older),
            vec!["Milk", "Eggs", "Bread", "Apples"],
            "device {d}"
        );
    }
}

#[test]
fn the_merge_is_the_same_whichever_list_a_third_device_hears_of_first() {
    for first in [0, 1] {
        let (c, older, _) = {
            let c = Cluster::new(3);
            let older = create_list(&c[0], "Groceries");
            task_in(&c[0], "Milk", Some(&older["id"]));
            let newer = create_list(&c[1], "groceries");
            task_in(&c[1], "Bread", Some(&newer["id"]));
            (c, older["id"].clone(), newer["id"].clone())
        };
        // C hears of the lists one at a time, in either order, and so makes its own merge.
        c.sync(first, 2);
        c.sync(1 - first, 2);
        c.sync_all();
        same_world(&c, 3);
        for d in 0..3 {
            let all = lists(&c[d]);
            assert_eq!(all.len(), 1, "device {d}, heard {first} first");
            assert_eq!(all[0].0, older.as_str().unwrap(), "the older survives");
            assert_eq!(titles_in(&c[d], &older).len(), 2);
        }
    }
}

#[test]
fn a_rename_into_another_lists_name_merges_the_same_way() {
    let c = Cluster::new(2);
    let alpha = create_list(&c[0], "Alpha");
    let beta = create_list(&c[0], "Beta");
    task_in(&c[0], "Alpha task", Some(&alpha["id"]));
    task_in(&c[0], "Beta task", Some(&beta["id"]));
    c.sync(0, 1);
    // Offline, each renames one of them to the same name.
    c[0].clock().advance(1_000);
    rename_list(&c[0], &alpha["id"], "Gamma");
    c[1].clock().advance(2_000);
    rename_list(&c[1], &beta["id"], "gamma");
    c.sync(0, 1);
    c.sync(0, 1);
    for d in 0..2 {
        let all = lists(&c[d]);
        assert_eq!(all.len(), 1, "device {d}");
        assert_eq!(all[0].0, alpha["id"].as_str().unwrap(), "Alpha is older");
        assert_eq!(
            titles_in(&c[d], &alpha["id"]),
            vec!["Alpha task", "Beta task"]
        );
        assert!(list_gone(&c[d], &beta["id"]));
    }
    same_world(&c, 2);
}

#[test]
fn a_rename_that_loses_the_race_still_leaves_one_list_when_the_older_is_the_one_renamed_later() {
    let c = Cluster::new(2);
    let alpha = create_list(&c[0], "Alpha");
    let beta = create_list(&c[0], "Beta");
    c.sync(0, 1);
    // The newer List (Beta) is renamed first; the older (Alpha) is renamed into the same name
    // later: whichever arrives, the older List survives.
    c[1].clock().advance(1_000);
    rename_list(&c[1], &beta["id"], "Home");
    c[0].clock().advance(5_000);
    rename_list(&c[0], &alpha["id"], "HOME");
    c.sync(1, 0);
    c.sync(1, 0);
    for d in 0..2 {
        let all = lists(&c[d]);
        assert_eq!(all.len(), 1, "device {d}");
        assert_eq!(all[0].0, alpha["id"].as_str().unwrap());
        assert_eq!(all[0].1, "HOME", "the survivor keeps its own name");
    }
    same_world(&c, 2);
}

#[test]
fn a_task_made_in_the_merged_away_list_by_a_device_that_had_not_heard_ends_in_the_survivor() {
    let c = Cluster::new(3);
    let older = create_list(&c[0], "Groceries");
    task_in(&c[0], "Milk", Some(&older["id"]));
    let newer = create_list(&c[1], "groceries");
    // C only knows the newer List, and adds a Task to it before the merge reaches it.
    c.sync(1, 2);
    let late = task_in(&c[2], "Late", Some(&newer["id"]));
    c.sync(0, 1);
    c.sync(0, 1);
    c.sync(2, 0);
    c.sync_all();
    for d in 0..3 {
        assert_eq!(
            task(&c[d], &late["id"])["list_id"],
            older["id"],
            "device {d}"
        );
    }
    same_world(&c, 3);
}

#[test]
fn a_merged_away_list_is_reported_with_the_name_of_the_device_it_came_from() {
    let (c, older, newer) = clash_on_creation();
    // B knows A as "Pixel 8".
    let a_id = c[0].device_id();
    c[1].with_db(|db| {
        db.execute(
            "INSERT INTO sync_peers(id, name, paired_at) VALUES (?1, 'Pixel 8', 1)",
            [&a_id],
        )
        .unwrap();
    });
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let sink = Arc::clone(&seen);
    tauri_specta::Event::listen(
        &c[1].handle(),
        move |e: tauri_specta::TypedEvent<crate::TodoListsMergedEvent>| {
            sink.lock()
                .unwrap()
                .push(serde_json::to_value(&e.payload).unwrap());
        },
    );
    c.sync(0, 1);
    c.sync(0, 1);
    let events = seen.lock().unwrap().clone();
    assert_eq!(events.len(), 1, "once: {events:?}");
    assert_eq!(events[0]["from_id"], newer);
    assert_eq!(events[0]["into_id"], older);
    assert_eq!(events[0]["from_name"], "GROCERIES");
    assert_eq!(events[0]["into_name"], "Groceries");
    assert_eq!(events[0]["device"], "Pixel 8");
}

// ---------------------------------------------------------------- order keys

#[test]
fn lists_and_tasks_with_equal_order_keys_are_ordered_by_id_on_every_device() {
    let c = Cluster::new(2);
    // From the same state both devices make the same key.
    let first = create_list(&c[0], "First");
    let second = create_list(&c[1], "Second");
    assert_eq!(first["sort_key"], second["sort_key"], "equal keys");
    let t1 = task_in(&c[0], "T1", None);
    std::thread::sleep(Duration::from_millis(3));
    let t2 = task_in(&c[1], "T2", None);
    assert_eq!(t1["sort_key"], t2["sort_key"]);
    c.sync(0, 1);
    c.sync(0, 1);
    for d in 0..2 {
        let names: Vec<String> = lists(&c[d]).into_iter().map(|(_, n)| n).collect();
        assert_eq!(names, vec!["First", "Second"], "device {d}");
        let order: Vec<String> = {
            let view: Value = c[d]
                .invoke("todo_inbox", json!({ "local": local() }))
                .unwrap();
            crate::test_support::inbox_tasks_of(&view)
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t["title"].as_str().unwrap().to_owned())
                .collect()
        };
        assert_eq!(order, vec!["T1", "T2"], "device {d}");
    }
}

#[test]
fn list_properties_edited_on_two_devices_merge_per_field() {
    let c = Cluster::new(2);
    let l = create_list(&c[0], "Work");
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>(
        "todo_list_set_emoji",
        json!({ "id": l["id"], "emoji": "💼" }),
    )
    .unwrap();
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>(
        "todo_list_set_group",
        json!({ "id": l["id"], "groupBy": "priority" }),
    )
    .unwrap();
    c[1].clock().advance(1_000);
    rename_list(&c[1], &l["id"], "Office");
    c.sync(0, 1);
    for d in 0..2 {
        let v: Value = c[d]
            .invoke("todo_list", json!({ "id": l["id"], "local": local() }))
            .unwrap();
        assert_eq!(v["list"]["emoji"], "💼", "device {d}");
        assert_eq!(v["list"]["group_by"], "priority");
        assert_eq!(v["list"]["name"], "Office");
    }
    same_world(&c, 2);
}

// ---------------------------------------------------------------- deletes

#[test]
fn a_task_moved_into_a_list_deleted_elsewhere_lands_in_the_inbox_on_both() {
    let c = Cluster::new(2);
    let l = create_list(&c[0], "Trip");
    let t = task_in(&c[0], "Pack", None);
    c.sync(0, 1);
    // A deletes the (empty) List; B, not knowing, moves the Task into it.
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>(
        "todo_list_delete",
        json!({ "id": l["id"], "choice": { "kind": "move_to_inbox" } }),
    )
    .unwrap();
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>(
        "todo_task_move",
        json!({ "id": t["id"], "listId": l["id"] }),
    )
    .unwrap();
    c.sync(0, 1);
    c.sync(0, 1);
    for d in 0..2 {
        assert_eq!(task(&c[d], &t["id"])["list_id"], Value::Null, "device {d}");
        assert_eq!(inbox_titles(&c[d]), vec!["Pack"]);
    }
    same_world(&c, 2);
}

#[test]
fn tasks_still_in_a_list_deleted_elsewhere_move_to_the_inbox_without_a_new_change() {
    let c = Cluster::new(2);
    let trip = create_list(&c[0], "Trip");
    c.sync(0, 1);
    // B adds a Task to the List that A then deletes (moving what A knows to the Inbox).
    let late = task_in(&c[1], "Late", Some(&trip["id"]));
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>(
        "todo_list_delete",
        json!({ "id": trip["id"], "choice": { "kind": "move_to_inbox" } }),
    )
    .unwrap();
    c.sync(0, 1);
    c.sync(0, 1);
    for d in 0..2 {
        assert_eq!(
            task(&c[d], &late["id"])["list_id"],
            Value::Null,
            "device {d}"
        );
    }
    let (to_b, to_a) = c.sync(0, 1);
    assert_eq!(
        (to_b.sent, to_a.sent),
        (0, 0),
        "nothing was recorded to tell the others"
    );
    same_world(&c, 2);
}

// ---------------------------------------------------------------- comments

#[test]
fn a_comment_added_to_a_task_deleted_elsewhere_stays_attached_and_returns_with_it() {
    let c = Cluster::new(2);
    let t = task_in(&c[0], "Plan weekend hike", None);
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    c[0].invoke::<()>("todo_task_delete", json!({ "id": t["id"] }))
        .unwrap();
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>(
        "todo_comment_add",
        json!({ "taskId": t["id"], "body": "Bring the stove" }),
    )
    .unwrap();
    c.sync(0, 1);
    c.sync(0, 1);
    for d in 0..2 {
        assert!(
            c[d].invoke::<Value>("todo_task", json!({ "id": t["id"] }))
                .is_err()
        );
        let back: Value = c[d]
            .invoke("todo_task_restore", json!({ "id": t["id"] }))
            .unwrap();
        assert_eq!(back["comment_count"], 1, "device {d}");
        let comments: Value = c[d]
            .invoke("todo_comments", json!({ "taskId": t["id"] }))
            .unwrap();
        assert_eq!(comments[0]["body"], "Bring the stove");
    }
}

#[test]
fn comments_sync_edits_deletes_and_the_count_is_recomputed() {
    let c = Cluster::new(2);
    let t = task_in(&c[0], "Report", None);
    let first: Value = c[0]
        .invoke(
            "todo_comment_add",
            json!({ "taskId": t["id"], "body": "one" }),
        )
        .unwrap();
    c[0].invoke::<Value>(
        "todo_comment_add",
        json!({ "taskId": t["id"], "body": "two" }),
    )
    .unwrap();
    c.sync(0, 1);
    assert_eq!(task(&c[1], &t["id"])["comment_count"], 2);
    c[1].clock().advance(1_000);
    let edited: Value = c[1]
        .invoke(
            "todo_comment_edit",
            json!({ "id": first["id"], "body": "one, edited" }),
        )
        .unwrap();
    assert_eq!(edited["edited"], true);
    c[0].clock().advance(2_000);
    let comments: Value = c[0]
        .invoke("todo_comments", json!({ "taskId": t["id"] }))
        .unwrap();
    let second = comments[1]["id"].clone();
    c[0].invoke::<()>("todo_comment_delete", json!({ "id": second }))
        .unwrap();
    c.sync(0, 1);
    for d in 0..2 {
        let comments: Value = c[d]
            .invoke("todo_comments", json!({ "taskId": t["id"] }))
            .unwrap();
        assert_eq!(comments.as_array().unwrap().len(), 1, "device {d}");
        assert_eq!(comments[0]["body"], "one, edited");
        assert_eq!(comments[0]["edited"], true);
        assert_eq!(task(&c[d], &t["id"])["comment_count"], 1);
    }
    same_world(&c, 2);
}

// ---------------------------------------------------------------- preferences

fn pref(app: &TestApp, key: &str) -> Value {
    let all: Value = app.invoke("settings_get_all", json!({})).unwrap();
    all["values"][key].clone()
}

#[test]
fn a_preference_changed_on_one_device_appears_on_the_other_and_the_theme_does_not() {
    let c = Cluster::new(2);
    c[0].set_pref("core.week_start", json!("sun"));
    c[0].set_pref("core.display_name", json!("Hemang"));
    c[0].set_pref("core.theme", json!("oled"));
    c[0].set_pref("todo.lists_sort", json!("name_asc"));
    c[0].set_pref("core.density", json!("compact"));
    // Old installs still have change-log rows for look and feel.
    c[0].with_db(|db| {
        mnemora_core::settings::record(db, "core", "theme", &json!("oled"), 1).unwrap();
    });
    c.sync(0, 1);
    assert_eq!(pref(&c[1], "core.week_start"), "sun");
    assert_eq!(pref(&c[1], "core.display_name"), "Hemang");
    assert_eq!(pref(&c[1], "todo.lists_sort"), "name_asc");
    assert_eq!(
        pref(&c[1], "core.theme"),
        "system",
        "theme stays per device"
    );
    assert_eq!(pref(&c[1], "core.density"), "comfortable");
    assert_eq!(pref(&c[0], "core.theme"), "oled");
    let (_, again) = c.sync(0, 1);
    assert_eq!(again.received, 0);
}

#[test]
fn look_and_feel_never_comes_in_either_even_when_a_peer_sends_it() {
    let c = Cluster::new(2);
    // B forges a record for the theme, as an older build or a hostile peer might.
    c[1].with_db(|db| {
        mnemora_core::settings::record(db, "core", "theme", &json!("dark"), 1).unwrap();
        mnemora_core::settings::record(db, "core", "week_start", &json!("sun"), 1).unwrap();
    });
    let (sent, _) = c.sync(1, 0);
    assert_eq!(sent.sent, 1, "only the week start is sent");
    assert_eq!(pref(&c[0], "core.theme"), "system");
    assert_eq!(pref(&c[0], "core.week_start"), "sun");
}

#[test]
fn the_newest_write_of_each_preference_wins_per_key() {
    let c = Cluster::new(2);
    c[0].set_pref("core.week_start", json!("sun"));
    c[0].set_pref("core.time_format", json!("12h"));
    c[1].clock().advance(5_000);
    c[1].set_pref("core.week_start", json!("mon"));
    c[1].set_pref("core.date_format", json!("iso"));
    c.sync(0, 1);
    for d in 0..2 {
        assert_eq!(
            pref(&c[d], "core.week_start"),
            "mon",
            "device {d}: B was later"
        );
        assert_eq!(pref(&c[d], "core.time_format"), "12h");
        assert_eq!(pref(&c[d], "core.date_format"), "iso");
    }
    same_world(&c, 2);
}

#[test]
fn a_synced_preference_announces_settings_changed_and_an_invalid_value_is_ignored() {
    let c = Cluster::new(2);
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let sink = Arc::clone(&seen);
    tauri_specta::Event::listen(
        &c[1].handle(),
        move |e: tauri_specta::TypedEvent<crate::SettingsChangedEvent>| {
            sink.lock()
                .unwrap()
                .push(serde_json::to_value(&e.payload).unwrap());
        },
    );
    c[0].set_pref("core.week_start", json!("sun"));
    c[0].with_db(|db| {
        // A value the key does not allow (a newer build, or garbage).
        mnemora_core::settings::record(db, "core", "time_format", &json!("37h"), 1).unwrap();
    });
    c.sync(0, 1);
    let events = seen.lock().unwrap().clone();
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["module"], "core");
    assert_eq!(events[0]["keys"], json!(["week_start"]));
    assert_eq!(pref(&c[1], "core.time_format"), "24h");
}

// ---------------------------------------------------------------- three devices, every order

fn log_rows(app: &TestApp) -> i64 {
    app.with_db(|db| {
        db.query_row("SELECT COUNT(*) FROM change_log", [], |r| r.get(0))
            .unwrap()
    })
}

/// Three devices each make "Groceries" with Tasks while apart; the pairs then meet in every
/// order (each pair once, and every four-step sequence of pairs). Every device must end with the
/// same Lists, Tasks, manual-order keys and Comments, and further rounds must record nothing.
#[test]
fn three_devices_making_the_same_list_converge_in_every_delivery_order() {
    let pairs = [(0, 1), (0, 2), (1, 2)];
    let mut sequences: Vec<Vec<usize>> = Vec::new();
    for a in 0..3 {
        for b in 0..3 {
            for c in 0..3 {
                if a != b && b != c && a != c {
                    sequences.push(vec![a, b, c]);
                }
                for d in 0..3 {
                    sequences.push(vec![a, b, c, d]);
                }
            }
        }
    }
    for order in sequences {
        let c = Cluster::new(3);
        let mut ids = Vec::new();
        for d in 0..3 {
            let l = create_list(&c[d], "Groceries");
            ids.push(l["id"].clone());
            for n in 0..2 {
                task_in(&c[d], &format!("d{d}t{n}"), Some(&l["id"]));
            }
        }
        for &p in &order {
            c.sync(pairs[p].0, pairs[p].1);
        }
        c.sync_all();
        c.sync_all();
        let rows: Vec<i64> = (0..3).map(|d| log_rows(&c[d])).collect();
        c.sync_all();
        c.sync_all();
        let after: Vec<i64> = (0..3).map(|d| log_rows(&c[d])).collect();
        assert_eq!(rows, after, "records keep growing after {order:?}");
        for d in 0..3 {
            let all = lists(&c[d]);
            assert_eq!(all.len(), 1, "device {d} after {order:?}");
            assert_eq!(all[0].0, ids[0].as_str().unwrap());
            assert_eq!(titles_in(&c[d], &ids[0]).len(), 6, "device {d} {order:?}");
        }
        same_world(&c, 3);
    }
}

// ---------------------------------------------------------------- restore after concurrent deletes

fn restore_task(app: &TestApp, id: &Value) {
    app.invoke::<Value>("todo_task_restore", json!({ "id": id }))
        .unwrap();
}

fn delete_task(app: &TestApp, id: &Value) {
    app.invoke::<Value>("todo_task_delete", json!({ "id": id }))
        .unwrap();
}

/// B makes a Task in a List that A deletes while B deletes the Task: the deleted Task's List
/// column may differ for a while, but Restore on either device (or both) must give every device
/// the same List.
#[test]
fn restoring_a_task_deleted_with_its_list_gives_the_same_list_on_every_device() {
    for restorers in [vec![0], vec![1], vec![0, 1]] {
        for (merged, delete_first) in [(false, false), (true, false), (true, true)] {
            let c = Cluster::new(3);
            let trip = create_list(&c[0], if merged { "Groceries" } else { "Trip" });
            let twin = merged.then(|| create_list(&c[1], "groceries"));
            if !merged {
                c.sync(0, 1);
            }
            // B (1) makes the Task in the List that is going away.
            let away = twin.as_ref().unwrap_or(&trip);
            let t = task_in(&c[1], "Pack", Some(&away["id"]));
            if merged {
                // The older List (A's) survives; B's newer List is merged away on its own.
                if delete_first {
                    delete_task(&c[1], &t["id"]);
                }
                c.sync(0, 1);
                if !delete_first {
                    delete_task(&c[1], &t["id"]);
                }
            } else {
                c[0].clock().advance(1_000);
                c[0].invoke::<Value>(
                    "todo_list_delete",
                    json!({ "id": trip["id"], "choice": { "kind": "move_to_inbox" } }),
                )
                .unwrap();
                c[1].clock().advance(2_000);
                delete_task(&c[1], &t["id"]);
            }
            c.sync_all();
            for &d in &restorers {
                restore_task(&c[d], &t["id"]);
            }
            c.sync_all();
            same_world(&c, 3);
            let lists: Vec<Value> = (0..3)
                .map(|d| task(&c[d], &t["id"])["list_id"].clone())
                .collect();
            assert!(
                lists.windows(2).all(|w| w[0] == w[1]),
                "{lists:?} restorers {restorers:?} merged {merged} first {delete_first}"
            );
        }
    }
}

/// A Task moved into a List that was deleted elsewhere goes where that List's name now points: a
/// List renamed to the deleted one's name later takes it in. Devices that hear of the rename
/// before or after the move agree.
#[test]
fn a_task_moved_into_a_deleted_list_lands_the_same_whether_a_rename_into_its_name_came_first() {
    for rename_first in [false, true] {
        let c = Cluster::new(3);
        let food = create_list(&c[0], "Food");
        let bar = create_list(&c[0], "Bar");
        let t = task_in(&c[0], "T", None);
        c.sync_all();
        c[0].invoke::<Value>(
            "todo_list_delete",
            json!({ "id": bar["id"], "choice": { "kind": "move_to_inbox" } }),
        )
        .unwrap();
        c[1].clock().advance(5_000);
        c[1].invoke::<Value>(
            "todo_task_move",
            json!({ "id": t["id"], "listId": bar["id"] }),
        )
        .unwrap();
        c.sync(0, 2);
        c[2].clock().advance(1_000);
        rename_list(&c[2], &food["id"], "Bar");
        if rename_first {
            c.sync(2, 1);
            c.sync(0, 1);
        } else {
            c.sync(0, 1);
            c.sync(2, 1);
        }
        c.sync_all();
        for d in 0..3 {
            assert_eq!(
                task(&c[d], &t["id"])["list_id"],
                food["id"],
                "device {d}, rename first: {rename_first}"
            );
        }
        same_world(&c, 3);
    }
}
