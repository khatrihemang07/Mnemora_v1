//! Sync scenarios for "Resolved automatically" on a [`Cluster`]: a value changed beside another is
//! decided newest-wins and logged once, the same on every device; "Use ‹other› instead" is an
//! ordinary new change; Order and Preference changes fold into one line per sync session; the
//! attention the Home card and the badge read. Every step is a real command; every assertion
//! reads back through commands (ADR 0009).

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::test_support::{Cluster, TestApp};

const TODAY: &str = "2026-10-22";

fn local() -> Value {
    json!({ "today": TODAY, "now": "14:05", "offset_min": 0 })
}

fn create(app: &TestApp, title: &str) -> Value {
    app.invoke("todo_task_create", json!({ "title": title }))
        .unwrap()
}

fn priority(app: &TestApp, id: &Value, p: u32) {
    app.invoke::<Value>("todo_task_set_priority", json!({ "id": id, "priority": p }))
        .unwrap();
}

fn task(app: &TestApp, id: &Value) -> Value {
    app.invoke("todo_task", json!({ "id": id })).unwrap()
}

fn history(app: &TestApp, id: &Value) -> Vec<String> {
    let lines: Value = app
        .invoke("todo_task_history", json!({ "id": id }))
        .unwrap();
    lines
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            format!(
                "{} {}",
                l["kind"].as_str().unwrap(),
                l["text"].as_str().unwrap()
            )
        })
        .collect()
}

/// "Resolved automatically" as a device lists it.
fn resolved(app: &TestApp) -> Vec<Value> {
    let list: Value = app.invoke("sync_conflicts", json!({})).unwrap();
    list["resolved"].as_array().unwrap().clone()
}

fn kind_of(app: &TestApp, kind: &str) -> Vec<Value> {
    resolved(app)
        .into_iter()
        .filter(|r| r["kind"] == kind)
        .collect()
}

fn attention(app: &TestApp) -> Value {
    app.invoke("sync_attention", json!({})).unwrap()
}

/// Two devices that know each other by name, with one Task they have both seen.
fn pair_with_task(title: &str) -> (Cluster, Value) {
    let c = Cluster::new(2);
    name_each_other(&c);
    let t = create(&c[0], title);
    c.sync(0, 1);
    (c, t["id"].clone())
}

fn name_each_other(c: &Cluster) {
    c[0].invoke::<()>("sync_rename", json!({ "name": "Mac" }))
        .unwrap();
    c[1].invoke::<()>("sync_rename", json!({ "name": "Pixel 8" }))
        .unwrap();
    for (me, other, name) in [(0, 1, "Pixel 8"), (1, 0, "Mac")] {
        let id = c[other].device_id();
        c[me].with_db(|db| {
            db.execute(
                "INSERT INTO sync_peers(id, name, paired_at) VALUES (?1, ?2, ?3)",
                (id.as_str(), name, Cluster::START),
            )
            .unwrap();
        });
    }
}

/// A Priority clash: the Mac sets P2 later, the Pixel 8 sets P3 meanwhile. P2 wins everywhere.
fn priority_clash() -> (Cluster, Value) {
    let (c, id) = pair_with_task("Water plants");
    c[0].clock().advance(5_000);
    priority(&c[0], &id, 2);
    c[1].clock().advance(1_000);
    priority(&c[1], &id, 3);
    c.sync(0, 1);
    (c, id)
}

// ---------------------------------------------------------------- one row, on both devices

#[test]
fn a_priority_clash_is_logged_once_the_same_on_both_devices() {
    let (c, _) = priority_clash();
    let a = kind_of(&c[0], "value");
    let b = kind_of(&c[1], "value");
    assert_eq!((a.len(), b.len()), (1, 1), "exactly one row on each");
    assert_eq!(a[0]["id"], b[0]["id"], "the same row");
    for row in [&a[0], &b[0]] {
        assert_eq!(row["title"], "‘Water plants’ · Priority");
        assert_eq!(row["detail"], "P2 (Mac) kept · P3 (Pixel 8) replaced");
        assert_eq!(row["action"], "Use P3 instead");
        assert_eq!(row["module"], "todo");
    }
}

#[test]
fn syncing_again_adds_nothing_and_whoever_decided_the_row_is_the_same() {
    let (c, _) = priority_clash();
    c.sync(0, 1);
    c.sync(1, 0);
    assert_eq!(kind_of(&c[0], "value").len(), 1);
    assert_eq!(kind_of(&c[1], "value").len(), 1);
    // A third device meeting both records through either hub makes the same row.
    let c3 = Cluster::new(3);
    name_each_other(&c3);
    let t = create(&c3[0], "Call bank")["id"].clone();
    c3.sync_all();
    c3[0].clock().advance(5_000);
    priority(&c3[0], &t, 2);
    c3[1].clock().advance(1_000);
    priority(&c3[1], &t, 3);
    c3.sync_all();
    let ids: Vec<Value> = (0..3)
        .map(|d| {
            let rows = kind_of(&c3[d], "value");
            assert_eq!(rows.len(), 1, "device {d}");
            rows[0]["id"].clone()
        })
        .collect();
    assert!(ids.iter().all(|i| *i == ids[0]), "{ids:?}");
}

#[test]
fn a_change_made_after_another_is_not_a_clash() {
    let (c, id) = pair_with_task("Water plants");
    c[0].clock().advance(1_000);
    priority(&c[0], &id, 2);
    c.sync(0, 1);
    c[1].clock().advance(2_000);
    priority(&c[1], &id, 3);
    c.sync(0, 1);
    assert_eq!(task(&c[0], &id)["priority"], 3);
    assert!(resolved(&c[0]).is_empty());
    assert!(resolved(&c[1]).is_empty());
}

#[test]
fn both_deleting_or_both_completing_a_task_is_not_a_clash_though_the_moments_differ() {
    let (c, id) = pair_with_task("Water plants");
    c[0].clock().advance(1_000);
    c[0].invoke::<()>("todo_task_delete", json!({ "id": id }))
        .unwrap();
    c[1].clock().advance(2_000);
    c[1].invoke::<()>("todo_task_delete", json!({ "id": id }))
        .unwrap();
    let (c2, id2) = pair_with_task("Call bank");
    for (d, ms) in [(0, 1_000), (1, 2_000)] {
        c2[d].clock().advance(ms);
        c2[d]
            .invoke::<Value>("todo_task_complete", json!({ "id": id2, "local": local() }))
            .unwrap();
    }
    c.sync(0, 1);
    c2.sync(0, 1);
    for d in 0..2 {
        assert!(kind_of(&c[d], "value").is_empty(), "delete, device {d}");
        assert!(kind_of(&c2[d], "value").is_empty(), "complete, device {d}");
    }
}

#[test]
fn a_task_status_that_gave_way_can_be_used_instead_unless_the_task_repeats() {
    let (c, id) = pair_with_task("Water plants");
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>("todo_task_complete", json!({ "id": id, "local": local() }))
        .unwrap();
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>("todo_task_wont_do", json!({ "id": id, "local": local() }))
        .unwrap();
    c.sync(0, 1);
    assert_eq!(task(&c[0], &id)["state"], "wont");
    let row = kind_of(&c[0], "value")[0].clone();
    assert_eq!(row["title"], "‘Water plants’ · Status");
    assert_eq!(row["action"], "Use Done instead");
    c[0].clock().advance(10_000);
    c[0].invoke::<()>("sync_resolution_use_other", json!({ "id": row["id"] }))
        .unwrap();
    assert_eq!(task(&c[0], &id)["state"], "done");
    c.sync(0, 1);
    assert_eq!(
        task(&c[1], &id)["state"],
        "done",
        "it synced as a normal edit"
    );
    assert_eq!(kind_of(&c[1], "value").len(), 1);
    assert_eq!(kind_of(&c[1], "value")[0]["used"], "Changed to Done");
}

#[test]
fn both_choosing_the_same_value_is_not_a_clash() {
    let (c, id) = pair_with_task("Water plants");
    c[0].clock().advance(1_000);
    priority(&c[0], &id, 2);
    c[1].clock().advance(2_000);
    priority(&c[1], &id, 2);
    c.sync(0, 1);
    assert!(resolved(&c[0]).is_empty());
    assert!(resolved(&c[1]).is_empty());
}

/// Both Mnemoras set the When of one Task: one only the Repeat, the other only the date.
fn when_beside(a_later: bool) -> (Cluster, Value) {
    let (c, id) = pair_with_task("Water plants");
    let set = |app: &TestApp, date: &str, every: u32| {
        let repeat = json!({ "freq": "day", "every": every, "days": null, "mode": "fixed" });
        app.invoke::<Value>(
            "todo_task_set_when",
            json!({ "id": id, "date": date, "time": null, "repeat": repeat, "local": local() }),
        )
        .unwrap();
    };
    set(&c[0], "2026-11-01", 1);
    c.sync(0, 1);
    let (late, early) = if a_later { (0, 1) } else { (1, 0) };
    c[early].clock().advance(1_000);
    c[late].clock().advance(5_000);
    set(&c[0], "2026-11-01", 2);
    set(&c[1], "2026-11-05", 1);
    c.sync(0, 1);
    (c, id)
}

#[test]
fn a_change_to_one_member_of_a_group_beside_another_member_is_a_logged_clash() {
    for a_later in [true, false] {
        let (c, id) = when_beside(a_later);
        let (mac, pixel) = (task(&c[0], &id), task(&c[1], &id));
        assert_eq!(
            mac["due_date"], pixel["due_date"],
            "converged, a_later={a_later}"
        );
        assert_eq!(
            mac["repeat_every"], pixel["repeat_every"],
            "whole group, a_later={a_later}"
        );
        let (a, b) = (kind_of(&c[0], "value"), kind_of(&c[1], "value"));
        assert_eq!(
            (a.len(), b.len()),
            (1, 1),
            "one row each, a_later={a_later}"
        );
        assert_eq!(a[0]["id"], b[0]["id"], "the same row, a_later={a_later}");
        // Use the other side: its whole group comes back, date and Repeat together.
        c[0].clock().advance(10_000);
        c[0].invoke::<()>("sync_resolution_use_other", json!({ "id": a[0]["id"] }))
            .unwrap();
        let after = task(&c[0], &id);
        let (date, every) = if a_later {
            ("2026-11-05", 1)
        } else {
            ("2026-11-01", 2)
        };
        assert_eq!(after["due_date"], date, "a_later={a_later}");
        assert_eq!(after["repeat_every"], every, "a_later={a_later}");
    }
}

#[test]
fn the_values_are_written_the_way_the_format_preferences_say() {
    let (c, id) = pair_with_task("Call bank");
    c[0].clock().advance(5_000);
    c[0].invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": id, "date": "2026-10-30", "time": "19:00", "repeat": null, "local": local() }),
    )
    .unwrap();
    c[1].clock().advance(1_000);
    c[1].invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": id, "date": "2026-10-28", "time": null, "repeat": null, "local": local() }),
    )
    .unwrap();
    c.sync(0, 1);
    let long = kind_of(&c[0], "value");
    assert_eq!(long[0]["title"], "‘Call bank’ · Due date");
    assert_eq!(
        long[0]["detail"],
        "Fri 30 Oct 19:00 (Mac) kept · Wed 28 Oct (Pixel 8) replaced"
    );
    c[0].set_pref("core.date_format", json!("iso"));
    c[0].set_pref("core.time_format", json!("12h"));
    assert_eq!(
        kind_of(&c[0], "value")[0]["detail"],
        "2026-10-30 7:00 PM (Mac) kept · 2026-10-28 (Pixel 8) replaced"
    );
    c[0].set_pref("core.date_format", json!("short"));
    assert_eq!(
        kind_of(&c[0], "value")[0]["detail"],
        "Oct 30, 2026 7:00 PM (Mac) kept · Oct 28, 2026 (Pixel 8) replaced"
    );
}

// ---------------------------------------------------------------- Use ‹other› instead

#[test]
fn use_other_is_an_ordinary_new_change_that_syncs_and_reads_as_a_history_line() {
    let (c, id) = priority_clash();
    let row = kind_of(&c[1], "value")[0].clone();
    // The Pixel 8 changes its mind: P3 after all.
    c[1].clock().advance(10_000);
    c[1].invoke::<()>("sync_resolution_use_other", json!({ "id": row["id"] }))
        .unwrap();
    assert_eq!(task(&c[1], &id)["priority"], 3);
    c.sync(0, 1);
    assert_eq!(
        task(&c[0], &id)["priority"],
        3,
        "it synced as a normal edit"
    );
    for d in 0..2 {
        let rows = kind_of(&c[d], "value");
        assert_eq!(
            rows.len(),
            1,
            "no second row for the correction, device {d}"
        );
        assert_eq!(rows[0]["used"], "Changed to P3", "device {d}");
        assert_eq!(rows[0]["action"], Value::Null, "device {d}");
        let lines = history(&c[d], &id);
        assert!(
            lines.first().unwrap() == "priority Priority P2 → P3",
            "a plain priority line: {lines:?}"
        );
    }
    // And it cannot be used twice.
    assert!(
        c[0].invoke::<()>("sync_resolution_use_other", json!({ "id": row["id"] }))
            .is_err()
    );
}

// ---------------------------------------------------------------- delete and edit, Inbox, deleted

#[test]
fn a_delete_and_an_edit_on_different_devices_are_both_kept_and_logged_once() {
    let (c, id) = pair_with_task("Water plants");
    c[0].clock().advance(1_000);
    c[0].invoke::<()>("todo_task_delete", json!({ "id": id }))
        .unwrap();
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>(
        "todo_task_update",
        json!({ "id": id, "title": "Water plants", "notes": "Fern first", "sessionId": "e" }),
    )
    .unwrap();
    c.sync(0, 1);
    c.sync(0, 1);
    for d in 0..2 {
        let rows = kind_of(&c[d], "delete_edit");
        assert_eq!(rows.len(), 1, "device {d}");
        assert_eq!(rows[0]["title"], "‘Water plants’ · Deleted");
        assert_eq!(
            rows[0]["detail"],
            "Deleted (Mac) kept · notes edited (Pixel 8) kept inside it"
        );
    }
    assert_eq!(
        kind_of(&c[0], "delete_edit")[0]["id"],
        kind_of(&c[1], "delete_edit")[0]["id"]
    );
}

/// Mac deletes and Pixel 8 edits, each without knowing of the other; `delete_first` says whose
/// clock is later.
fn delete_beside_edit(delete_first: bool) -> (Cluster, Value) {
    let (c, id) = pair_with_task("Water plants");
    let (delete_at, edit_at) = if delete_first {
        (1_000, 2_000)
    } else {
        (2_000, 1_000)
    };
    c[0].clock().advance(delete_at);
    c[0].invoke::<()>("todo_task_delete", json!({ "id": id }))
        .unwrap();
    c[1].clock().advance(edit_at);
    c[1].invoke::<Value>(
        "todo_task_update",
        json!({ "id": id, "title": "Water plants", "notes": "Fern first", "sessionId": "e" }),
    )
    .unwrap();
    (c, id)
}

#[test]
fn a_delete_and_an_edit_are_logged_whichever_clock_is_later() {
    for delete_first in [true, false] {
        let (c, _) = delete_beside_edit(delete_first);
        c.sync(0, 1);
        c.sync(0, 1);
        for d in 0..2 {
            let rows = kind_of(&c[d], "delete_edit");
            assert_eq!(rows.len(), 1, "device {d}, delete first: {delete_first}");
            assert_eq!(
                rows[0]["detail"], "Deleted (Mac) kept · notes edited (Pixel 8) kept inside it",
                "device {d}, delete first: {delete_first}"
            );
        }
        assert_eq!(
            kind_of(&c[0], "delete_edit")[0]["id"],
            kind_of(&c[1], "delete_edit")[0]["id"]
        );
    }
}

#[test]
fn an_edit_the_deleting_device_had_already_received_is_not_logged() {
    let (c, id) = pair_with_task("Water plants");
    c[1].clock().advance(1_000);
    c[1].invoke::<Value>(
        "todo_task_update",
        json!({ "id": id, "title": "Water plants", "notes": "Fern first", "sessionId": "e" }),
    )
    .unwrap();
    c.sync(0, 1);
    // The Mac now knows the edit, and deletes after it; its clock is later.
    c[0].clock().advance(2_000);
    c[0].invoke::<()>("todo_task_delete", json!({ "id": id }))
        .unwrap();
    c.sync(0, 1);
    c.sync(0, 1);
    for d in 0..2 {
        assert!(kind_of(&c[d], "delete_edit").is_empty(), "device {d}");
    }
}

#[test]
fn a_delete_and_an_earlier_edit_met_through_a_third_device_are_logged_once_in_any_order() {
    let c = Cluster::new(3);
    let names = ["Mac", "Pixel 8", "Tablet"];
    for (i, name) in names.iter().enumerate() {
        c[i].invoke::<()>("sync_rename", json!({ "name": name }))
            .unwrap();
        for (j, other) in names.iter().enumerate().filter(|(j, _)| *j != i) {
            let id = c[j].device_id();
            c[i].with_db(|db| {
                db.execute(
                    "INSERT INTO sync_peers(id, name, paired_at) VALUES (?1, ?2, ?3)",
                    (id.as_str(), *other, Cluster::START),
                )
                .unwrap();
            });
        }
    }
    let id = create(&c[0], "Water plants")["id"].clone();
    c.sync_all();
    c[1].clock().advance(1_000);
    c[1].invoke::<Value>(
        "todo_task_update",
        json!({ "id": id, "title": "Water plants", "notes": "Fern first", "sessionId": "e" }),
    )
    .unwrap();
    c[0].clock().advance(2_000);
    c[0].invoke::<()>("todo_task_delete", json!({ "id": id }))
        .unwrap();
    // The Tablet hears of the delete first, and the edit reaches it later.
    c.sync(0, 2);
    c.sync(1, 2);
    c.sync_all();
    for d in 0..3 {
        let rows = kind_of(&c[d], "delete_edit");
        assert_eq!(rows.len(), 1, "device {d}");
        assert_eq!(
            rows[0]["detail"],
            "Deleted (Mac) kept · notes edited (Pixel 8) kept inside it"
        );
    }
}

#[test]
fn a_delete_met_by_two_edits_is_one_row_the_same_on_three_devices_in_any_order() {
    let c = Cluster::new(3);
    let names = ["Mac", "Pixel 8", "Tablet"];
    for (i, name) in names.iter().enumerate() {
        c[i].invoke::<()>("sync_rename", json!({ "name": name }))
            .unwrap();
        for (j, other) in names.iter().enumerate().filter(|(j, _)| *j != i) {
            let id = c[j].device_id();
            c[i].with_db(|db| {
                db.execute(
                    "INSERT INTO sync_peers(id, name, paired_at) VALUES (?1, ?2, ?3)",
                    (id.as_str(), *other, Cluster::START),
                )
                .unwrap();
            });
        }
    }
    let id = create(&c[0], "Water plants")["id"].clone();
    c.sync_all();
    c[0].clock().advance(1_000);
    c[0].invoke::<()>("todo_task_delete", json!({ "id": id }))
        .unwrap();
    for (d, at, notes) in [(1, 2_000, "Fern first"), (2, 3_000, "Skip the cactus")] {
        c[d].clock().advance(at);
        c[d].invoke::<Value>(
            "todo_task_update",
            json!({ "id": id, "title": "Water plants", "notes": notes, "sessionId": format!("e{d}") }),
        )
        .unwrap();
    }
    // The later edit is met first on the Tablet and the Mac; the earlier one reaches each late.
    c.sync(0, 2);
    c.sync(0, 1);
    c.sync(1, 2);
    c.sync_all();
    let rows: Vec<Value> = (0..3)
        .map(|d| {
            let rows = kind_of(&c[d], "delete_edit");
            assert_eq!(rows.len(), 1, "device {d}");
            rows[0].clone()
        })
        .collect();
    for row in &rows {
        assert_eq!(row["id"], rows[0]["id"]);
        assert_eq!(row["detail"], rows[0]["detail"]);
        assert_eq!(row["at"], rows[0]["at"]);
    }
    assert_eq!(
        rows[0]["detail"],
        "Deleted (Mac) kept · notes edited (Pixel 8) kept inside it"
    );
}

#[test]
fn a_task_moved_into_a_list_deleted_elsewhere_lands_in_the_inbox_and_is_logged() {
    let c = Cluster::new(2);
    name_each_other(&c);
    let list: Value = c[0]
        .invoke("todo_list_create", json!({ "name": "Trip" }))
        .unwrap();
    let t = create(&c[0], "Pack");
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>(
        "todo_list_delete",
        json!({ "id": list["id"], "choice": { "kind": "move_to_inbox" } }),
    )
    .unwrap();
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>(
        "todo_task_move",
        json!({ "id": t["id"], "listId": list["id"] }),
    )
    .unwrap();
    c.sync(0, 1);
    c.sync(0, 1);
    for d in 0..2 {
        let rows = kind_of(&c[d], "task_inbox");
        assert_eq!(rows.len(), 1, "device {d}");
        assert_eq!(rows[0]["title"], "‘Pack’ moved to Inbox");
        // The device that deleted the List is named, whichever device is looking.
        assert_eq!(rows[0]["detail"], "Trip was deleted on Mac", "device {d}");
    }
}

#[test]
fn a_text_clash_on_a_deleted_entry_is_logged_without_asking() {
    let c = Cluster::new(2);
    name_each_other(&c);
    let id = c[0]
        .invoke::<Value>(
            "journal_entry_create",
            json!({ "entry": {
                "page_date": "2026-10-21", "text": "Went for a run.\n", "written_tz": "UTC",
                "written_offset_min": 0, "session_id": "create",
            }}),
        )
        .unwrap()["id"]
        .clone();
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>(
        "journal_entry_update",
        json!({ "id": id, "text": "Went for a swim.\n", "sessionId": "a" }),
    )
    .unwrap();
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>(
        "journal_entry_update",
        json!({ "id": id, "text": "Went for a cycle.\n", "sessionId": "b" }),
    )
    .unwrap();
    c[1].clock().advance(3_000);
    c[1].invoke::<()>("journal_entry_delete", json!({ "id": id }))
        .unwrap();
    c.sync(0, 1);
    c.sync(0, 1);
    for d in 0..2 {
        let conflicts: Value = c[d].invoke("sync_conflicts", json!({})).unwrap();
        assert!(
            conflicts["needs_you"].as_array().unwrap().is_empty(),
            "device {d}"
        );
        assert_eq!(kind_of(&c[d], "conflict_deleted").len(), 1, "device {d}");
    }
}

// ---------------------------------------------------------------- Order and Preferences

#[test]
fn order_and_preference_changes_fold_into_one_line_per_sync_session() {
    let c = Cluster::new(2);
    name_each_other(&c);
    let one = c[0]
        .invoke::<Value>("todo_list_create", json!({ "name": "One" }))
        .unwrap();
    let two = c[0]
        .invoke::<Value>("todo_list_create", json!({ "name": "Two" }))
        .unwrap();
    c.sync(0, 1);
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>(
        "todo_list_reorder",
        json!({ "id": two["id"], "before": null, "after": one["id"] }),
    )
    .unwrap();
    c[0].set_pref("core.week_start", json!("sun"));
    c[0].set_pref("core.time_format", json!("12h"));
    c.sync(0, 1);
    let lines = kind_of(&c[1], "order_prefs");
    assert_eq!(lines.len(), 1, "one line for the session");
    assert_eq!(lines[0]["title"], "Order & preferences · 3 changes");
    assert_eq!(lines[0]["detail"], "Mac");
    assert_eq!(lines[0]["changes"].as_array().unwrap().len(), 3);
    assert!(
        kind_of(&c[0], "order_prefs").is_empty(),
        "the sender took nothing"
    );

    // Another sync with more changes is another line; an empty sync adds none.
    c[0].clock().advance(1_000);
    c[0].set_pref("core.week_start", json!("mon"));
    c.sync(0, 1);
    c.sync(0, 1);
    let lines = kind_of(&c[1], "order_prefs");
    assert_eq!(lines.len(), 2);
    assert!(
        lines
            .iter()
            .any(|l| l["title"] == "Order & preferences · 1 change")
    );
    // They are this device's own account of a session: nothing of them travels.
    assert!(kind_of(&c[0], "order_prefs").is_empty());
}

// ---------------------------------------------------------------- since your last visit

#[test]
fn rows_are_new_until_the_log_is_visited_and_new_again_when_more_arrive() {
    let (c, id) = priority_clash();
    assert_eq!(resolved(&c[1])[0]["fresh"], true);
    c[1].invoke::<()>("sync_resolutions_visited", json!({}))
        .unwrap();
    assert_eq!(resolved(&c[1])[0]["fresh"], false, "seen now");
    c[0].clock().advance(60_000);
    priority(&c[0], &id, 1);
    c[1].clock().advance(1_000);
    priority(&c[1], &id, 4);
    c.sync(0, 1);
    let rows = resolved(&c[1]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows.iter().filter(|r| r["fresh"] == true).count(), 1);
}

// ---------------------------------------------------------------- what needs attention

#[test]
fn attention_follows_the_conflicts_that_wait() {
    let c = Cluster::new(2);
    name_each_other(&c);
    assert_eq!(attention(&c[0])["needed"], false);
    let id = c[0]
        .invoke::<Value>(
            "journal_entry_create",
            json!({ "entry": {
                "page_date": "2026-10-21", "text": "Went for a run.\n", "written_tz": "UTC",
                "written_offset_min": 0, "session_id": "create",
            }}),
        )
        .unwrap()["id"]
        .clone();
    c.sync(0, 1);
    for (d, text, ms) in [
        (0, "Went for a swim.\n", 1_000),
        (1, "Went for a cycle.\n", 2_000),
    ] {
        c[d].clock().advance(ms);
        c[d].invoke::<Value>(
            "journal_entry_update",
            json!({ "id": id, "text": text, "sessionId": format!("s{d}") }),
        )
        .unwrap();
    }
    c.sync(0, 1);
    for d in 0..2 {
        let a = attention(&c[d]);
        assert_eq!(a["conflicts"], 1, "device {d}");
        assert_eq!(a["needed"], true, "device {d}");
    }
    let waiting: Value = c[0].invoke("sync_conflicts", json!({})).unwrap();
    c[0].invoke::<()>(
        "sync_conflict_resolve",
        json!({ "id": waiting["needs_you"][0]["id"], "choice": { "kind": "keep_this" } }),
    )
    .unwrap();
    c.sync(0, 1);
    for d in 0..2 {
        assert_eq!(attention(&c[d])["needed"], false, "device {d}");
    }
}

// ---------------------------------------------------------------- the toast

#[test]
fn a_new_conflict_is_announced_only_on_a_device_set_to_a_toast() {
    let c = Cluster::new(2);
    name_each_other(&c);
    // The Pixel 8 wants toasts, the Mac stays quiet (the default).
    c[1].invoke::<()>(
        "sync_set_option",
        json!({ "option": "toast_conflicts", "enabled": true }),
    )
    .unwrap();
    let heard: Arc<Mutex<Vec<(usize, Value)>>> = Arc::default();
    for d in 0..2 {
        let sink = Arc::clone(&heard);
        tauri_specta::Event::listen(
            &c[d].handle(),
            move |e: tauri_specta::TypedEvent<crate::SyncNewConflictsEvent>| {
                sink.lock()
                    .unwrap()
                    .push((d, serde_json::to_value(&e.payload).unwrap()));
            },
        );
    }
    let id = c[0]
        .invoke::<Value>(
            "journal_entry_create",
            json!({ "entry": {
                "page_date": "2026-10-21", "text": "Went for a run.\n", "written_tz": "UTC",
                "written_offset_min": 0, "session_id": "create",
            }}),
        )
        .unwrap()["id"]
        .clone();
    c.sync(0, 1);
    for (d, text, ms) in [
        (0, "Went for a swim.\n", 1_000),
        (1, "Went for a cycle.\n", 2_000),
    ] {
        c[d].clock().advance(ms);
        c[d].invoke::<Value>(
            "journal_entry_update",
            json!({ "id": id, "text": text, "sessionId": format!("s{d}") }),
        )
        .unwrap();
    }
    c.sync(0, 1);
    c.sync(0, 1);
    let heard = heard.lock().unwrap().clone();
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert_eq!(heard[0].0, 1, "only the device set to a toast");
    assert_eq!(heard[0].1["count"], 1);
    assert_eq!(heard[0].1["device"], "Mac");
}
