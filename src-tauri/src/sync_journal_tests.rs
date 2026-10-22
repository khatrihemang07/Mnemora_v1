//! Sync scenarios for Journal Entries, and for changes held back while a person is typing, on a
//! [`Cluster`]: several devices, each a `TestApp` with its own database, synced pairwise through
//! the sync crate's in-memory transport. Every step is a real command; every assertion reads back
//! through commands (ADR 0009).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::{Value, json};

use crate::test_support::{Cluster, TestApp};

const ENTRY: &str = "journal_entry";
const TASK: &str = "todo_task";

fn create(app: &TestApp, date: &str, text: &str) -> Value {
    app.invoke(
        "journal_entry_create",
        json!({ "entry": {
            "page_date": date, "text": text, "written_tz": "Asia/Kolkata",
            "written_offset_min": 330, "session_id": format!("c-{text}"),
        }}),
    )
    .unwrap()
}

fn edit(app: &TestApp, id: &Value, text: &str, session: &str) -> Value {
    app.invoke(
        "journal_entry_update",
        json!({ "id": id, "text": text, "sessionId": session }),
    )
    .unwrap()
}

fn move_to(app: &TestApp, id: &Value, date: &str) {
    app.invoke::<Value>("journal_entry_move", json!({ "id": id, "pageDate": date }))
        .unwrap();
}

fn delete(app: &TestApp, id: &Value) {
    app.invoke::<()>("journal_entry_delete", json!({ "id": id }))
        .unwrap();
}

fn restore(app: &TestApp, id: &Value) {
    app.invoke::<Value>("journal_entry_restore", json!({ "id": id }))
        .unwrap();
}

/// Every live Entry as the Journal shows it: `[date, body]`, newest Page first.
fn shown(app: &TestApp) -> Vec<(String, String)> {
    let pages: Vec<Value> = app
        .invoke(
            "journal_pages",
            json!({ "cursor": null, "limit": 50, "direction": "older" }),
        )
        .unwrap();
    pages
        .iter()
        .flat_map(|p| p["entries"].as_array().unwrap().iter())
        .map(|e| {
            (
                e["page_date"].as_str().unwrap().to_owned(),
                e["body"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// The Entry with `id` as the Journal shows it, if it is live.
fn entry(app: &TestApp, id: &Value) -> Option<Value> {
    let pages: Vec<Value> = app
        .invoke(
            "journal_pages",
            json!({ "cursor": null, "limit": 50, "direction": "older" }),
        )
        .unwrap();
    pages
        .iter()
        .flat_map(|p| p["entries"].as_array().unwrap().iter())
        .find(|e| e["id"] == *id)
        .cloned()
}

fn body(app: &TestApp, id: &Value) -> Option<String> {
    entry(app, id).map(|e| e["body"].as_str().unwrap().to_owned())
}

fn hold(app: &TestApp, entity: &str, id: &Value, scope: &str) {
    app.invoke::<bool>(
        "editing_hold",
        json!({ "entity": entity, "entityId": id, "scope": scope }),
    )
    .unwrap();
}

fn release(app: &TestApp, entity: &str, id: &Value, scope: &str) {
    app.invoke::<()>(
        "editing_release",
        json!({ "entity": entity, "entityId": id, "scope": scope }),
    )
    .unwrap();
}

/// Counts the Journal changed events a device sends to its screens.
fn journal_events(app: &TestApp) -> Arc<AtomicU32> {
    let seen = Arc::new(AtomicU32::new(0));
    let counter = seen.clone();
    tauri_specta::Event::listen(
        &app.handle(),
        move |_: tauri_specta::TypedEvent<crate::JournalChangedEvent>| {
            counter.fetch_add(1, Ordering::SeqCst);
        },
    );
    seen
}

/// Two devices that have synced one Entry.
fn pair_with_entry(text: &str) -> (Cluster, Value) {
    let c = Cluster::new(2);
    let e = create(&c[0], "2026-10-20", text);
    c.sync(0, 1);
    (c, e["id"].clone())
}

// ---------------------------------------------------------------- Entries travel

#[test]
fn an_entry_created_moved_edited_and_deleted_on_one_device_does_the_same_on_the_other() {
    let c = Cluster::new(2);
    let made = create(&c[0], "2026-10-20", "Walked by the **river**");
    let id = made["id"].clone();
    let (sent, got) = c.sync(0, 1);
    assert_eq!((sent.sent, got.received), (1, 1));
    // As it was written: the same Entry, the written moment and zone included.
    let mut there = entry(&c[1], &id).unwrap();
    // `updated_at` is when each device last changed its own row.
    there["updated_at"] = made["updated_at"].clone();
    assert_eq!(there, made);

    c[0].clock().advance(1_000);
    move_to(&c[0], &id, "2026-10-22");
    c.sync(0, 1);
    assert_eq!(
        shown(&c[1]),
        [("2026-10-22".into(), "Walked by the **river**".into())]
    );

    c[0].clock().advance(1_000);
    edit(&c[0], &id, "Walked by the river, then home", "s1");
    c.sync(0, 1);
    let there = entry(&c[1], &id).unwrap();
    assert_eq!(there["body"], "Walked by the river, then home");
    assert_eq!(
        there["page_date"], "2026-10-22",
        "the move survived the edit"
    );
    assert_eq!(there["word_count"], 6, "recomputed on this device");

    c[0].clock().advance(1_000);
    delete(&c[0], &id);
    c.sync(0, 1);
    assert!(entry(&c[1], &id).is_none());
    assert!(shown(&c[1]).is_empty());

    c[0].clock().advance(1_000);
    restore(&c[0], &id);
    c.sync(0, 1);
    assert_eq!(
        body(&c[1], &id).as_deref(),
        Some("Walked by the river, then home")
    );
}

#[test]
fn the_edited_time_is_derived_from_when_the_text_was_changed() {
    let (c, id) = pair_with_entry("one two");
    // A typo fix right after writing is not an edit.
    c[0].clock().advance(60_000);
    edit(&c[0], &id, "one two three", "s1");
    c.sync(0, 1);
    assert_eq!(entry(&c[1], &id).unwrap()["edited_at"], Value::Null);
    // An hour later it is, at the moment it was made.
    c[0].clock().advance(3_600_000);
    edit(&c[0], &id, "one two three four", "s2");
    c.sync(0, 1);
    let (here, there) = (entry(&c[0], &id).unwrap(), entry(&c[1], &id).unwrap());
    assert_eq!(there["edited_at"], here["edited_at"]);
    assert!(there["edited_at"].is_i64());
    assert_eq!(there["word_count"], 4);
    // Ticking a checklist item is never an edit.
    let (c, id) = pair_with_entry("- [ ] milk");
    c[0].clock().advance(3_600_000);
    c[0].invoke::<Value>("journal_entry_toggle_task", json!({ "id": id, "line": 0 }))
        .unwrap();
    c.sync(0, 1);
    assert_eq!(body(&c[1], &id).as_deref(), Some("- [x] milk"));
    assert_eq!(entry(&c[1], &id).unwrap()["edited_at"], Value::Null);
}

#[test]
fn the_page_date_and_the_text_changed_on_two_devices_both_survive() {
    let (c, id) = pair_with_entry("morning");
    c[0].clock().advance(1_000);
    move_to(&c[0], &id, "2026-10-25");
    c[1].clock().advance(2_000);
    edit(&c[1], &id, "morning, edited", "s1");
    c.sync(0, 1);
    for d in 0..2 {
        assert_eq!(
            shown(&c[d]),
            [("2026-10-25".into(), "morning, edited".into())],
            "device {d}"
        );
    }
}

#[test]
fn a_delete_and_an_edit_on_two_devices_leave_it_deleted_with_the_new_text() {
    let (c, id) = pair_with_entry("keep me");
    c[0].clock().advance(1_000);
    delete(&c[0], &id);
    c[1].clock().advance(2_000);
    edit(&c[1], &id, "keep me, edited", "s1");
    c.sync(0, 1);
    for d in 0..2 {
        assert!(entry(&c[d], &id).is_none(), "device {d} still deleted");
    }
    restore(&c[0], &id);
    c.sync(0, 1);
    assert_eq!(body(&c[1], &id).as_deref(), Some("keep me, edited"));
}

#[test]
fn remote_entry_changes_refresh_open_journal_views_through_one_event_per_batch() {
    let c = Cluster::new(2);
    let events = journal_events(&c[1]);
    create(&c[0], "2026-10-20", "hello");
    c.sync(0, 1);
    assert_eq!(events.load(Ordering::SeqCst), 1);
    c.sync(0, 1);
    assert_eq!(
        events.load(Ordering::SeqCst),
        1,
        "nothing new, nothing sent"
    );
}

// ---------------------------------------------------------------- held while editing

#[test]
fn a_remote_edit_to_an_entry_being_edited_is_applied_only_after_the_session_ends() {
    let (c, id) = pair_with_entry("first draft");
    let events = journal_events(&c[1]);
    hold(&c[1], ENTRY, &id, "*");
    c[0].clock().advance(1_000);
    edit(&c[0], &id, "written on the other device", "s-a");
    c.sync(0, 1);
    assert_eq!(
        body(&c[1], &id).as_deref(),
        Some("first draft"),
        "the editor does not jump"
    );
    assert_eq!(
        events.load(Ordering::SeqCst),
        0,
        "nothing changed on screen"
    );

    release(&c[1], ENTRY, &id, "*");
    assert_eq!(
        body(&c[1], &id).as_deref(),
        Some("written on the other device")
    );
    assert_eq!(
        events.load(Ordering::SeqCst),
        1,
        "the screen refreshes once, silently"
    );
    // It was not a new change of this device's: nothing to send back, and no gap either.
    let (a, b) = c.sync(0, 1);
    assert_eq!((a.sent, b.sent), (0, 0));
    assert_eq!(body(&c[0], &id), body(&c[1], &id));
}

#[test]
fn text_saved_during_the_session_after_the_remote_edit_arrived_stands_on_both_devices() {
    let (c, id) = pair_with_entry("first draft");
    hold(&c[1], ENTRY, &id, "*");
    c[0].clock().advance(1_000);
    edit(&c[0], &id, "the other device's words", "s-a");
    c.sync(0, 1);
    // The person goes on typing: the editor saves under its own session.
    c[1].clock().advance(2_000);
    edit(&c[1], &id, "my words, still typing", "s-b");
    assert_eq!(body(&c[1], &id).as_deref(), Some("my words, still typing"));
    release(&c[1], ENTRY, &id, "*");
    assert_eq!(
        body(&c[1], &id).as_deref(),
        Some("my words, still typing"),
        "the later text wins and is not overwritten by the held one"
    );
    c.sync(0, 1);
    assert_eq!(body(&c[0], &id).as_deref(), Some("my words, still typing"));
}

#[test]
fn what_was_held_converges_with_other_changes_that_came_in_meanwhile() {
    let (c, id) = pair_with_entry("first draft");
    hold(&c[1], ENTRY, &id, "*");
    c[0].clock().advance(1_000);
    edit(&c[0], &id, "A edits", "s-a");
    move_to(&c[0], &id, "2026-10-27");
    c.sync(0, 1);
    // The page date and the text are both held: the Entry stays where it was.
    assert_eq!(shown(&c[1]), [("2026-10-20".into(), "first draft".into())]);
    release(&c[1], ENTRY, &id, "*");
    assert_eq!(shown(&c[1]), [("2026-10-27".into(), "A edits".into())]);
    c.sync_all();
    assert_eq!(shown(&c[0]), shown(&c[1]));
}

#[test]
fn a_device_holding_an_entry_relays_what_it_holds_and_never_claims_it_as_its_own() {
    let c = Cluster::new(3);
    let e = create(&c[0], "2026-10-20", "first draft");
    let id = e["id"].clone();
    c.sync_all();
    hold(&c[1], ENTRY, &id, "*");
    c[0].clock().advance(1_000);
    edit(&c[0], &id, "A's new words", "s-a");
    c.sync(0, 1);
    assert_eq!(body(&c[1], &id).as_deref(), Some("first draft"));
    // The holding device changes something else, then passes everything on before it lets go.
    c[1].clock().advance(1_000);
    move_to(&c[1], &id, "2026-10-27");
    c.sync(1, 2);
    assert_eq!(
        shown(&c[2]),
        [("2026-10-27".into(), "A's new words".into())],
        "the relayed text is A's, not the stale text the holder's database has"
    );
    release(&c[1], ENTRY, &id, "*");
    c.sync_all();
    for d in 0..3 {
        assert_eq!(
            shown(&c[d]),
            [("2026-10-27".into(), "A's new words".into())],
            "device {d}"
        );
    }
}

#[test]
fn what_was_held_gives_the_same_edited_time_as_when_it_was_not_held() {
    let c = Cluster::new(3);
    let e = create(&c[0], "2026-10-20", "first draft");
    let id = e["id"].clone();
    c.sync_all();
    hold(&c[1], ENTRY, &id, "*");
    c[0].clock().advance(20 * 60 * 1000);
    edit(&c[0], &id, "written much later", "s-a");
    c[0].clock().advance(5 * 60 * 1000);
    move_to(&c[0], &id, "2026-10-27");
    c.sync(0, 1);
    c.sync(0, 2);
    release(&c[1], ENTRY, &id, "*");
    let edited = |d: usize| entry(&c[d], &id).unwrap()["edited_at"].clone();
    assert!(edited(2).is_number());
    assert_eq!(
        edited(1),
        edited(2),
        "held or not, the text was edited at the same moment"
    );
    assert_eq!(edited(0), edited(2));
}

#[test]
fn a_hold_whose_editor_vanished_ends_when_the_lease_lapses() {
    let (c, id) = pair_with_entry("first draft");
    hold(&c[1], ENTRY, &id, "*");
    c[0].clock().advance(1_000);
    edit(&c[0], &id, "from the other device", "s-a");
    c.sync(0, 1);
    assert_eq!(body(&c[1], &id).as_deref(), Some("first draft"));
    // The editor was never closed properly: two minutes pass without it renewing the hold.
    c[1].with_db(|db| {
        db.execute(
            "UPDATE editing_sessions SET touched_at = touched_at - 121000",
            [],
        )
        .unwrap();
    });
    // The app coming back to the foreground merges what waited.
    c[1].invoke::<()>("sync_resume", json!({})).unwrap();
    assert_eq!(body(&c[1], &id).as_deref(), Some("from the other device"));
}

#[test]
fn a_lapsed_hold_also_ends_when_the_next_sync_batch_arrives() {
    let (c, id) = pair_with_entry("first draft");
    hold(&c[1], ENTRY, &id, "*");
    c[0].clock().advance(1_000);
    edit(&c[0], &id, "from the other device", "s-a");
    c.sync(0, 1);
    c[1].with_db(|db| {
        db.execute("UPDATE editing_sessions SET touched_at = 0", [])
            .unwrap();
    });
    c.sync(0, 1);
    assert_eq!(body(&c[1], &id).as_deref(), Some("from the other device"));
}

#[test]
fn holds_never_survive_a_restart_and_what_waited_is_merged_at_start() {
    let (c, id) = pair_with_entry("first draft");
    hold(&c[1], ENTRY, &id, "*");
    c[0].clock().advance(1_000);
    edit(&c[0], &id, "from the other device", "s-a");
    c.sync(0, 1);
    assert_eq!(body(&c[1], &id).as_deref(), Some("first draft"));
    c[1].restart();
    assert_eq!(body(&c[1], &id).as_deref(), Some("from the other device"));
    let holds: i64 = c[1].with_db(|db| {
        db.query_row("SELECT COUNT(*) FROM editing_sessions", [], |r| r.get(0))
            .unwrap()
    });
    assert_eq!(holds, 0);
}

#[test]
fn with_sync_off_the_editor_is_told_there_is_nothing_to_keep_renewing() {
    let off = TestApp::new();
    let e = create(&off, "2026-10-20", "first draft");
    let ask = |app: &TestApp| -> bool {
        app.invoke(
            "editing_hold",
            json!({ "entity": ENTRY, "entityId": e["id"], "scope": "*" }),
        )
        .unwrap()
    };
    assert!(!ask(&off));
    assert!(ask(&Cluster::new(1)[0]));
}

#[test]
fn an_editor_can_only_hold_something_that_exists_to_be_edited() {
    let c = Cluster::new(1);
    let err = c[0]
        .invoke::<()>(
            "editing_hold",
            json!({ "entity": "no_such", "entityId": "x", "scope": "*" }),
        )
        .unwrap_err();
    assert_eq!(err["code"], "validation");
    let err = c[0]
        .invoke::<()>(
            "editing_hold",
            json!({ "entity": TASK, "entityId": "x", "scope": "no_such_field" }),
        )
        .unwrap_err();
    assert_eq!(err["code"], "validation");
}

// ---------------------------------------------------------------- Task sheet fields

#[test]
fn in_an_open_task_sheet_other_fields_refresh_while_the_field_being_typed_in_waits() {
    let c = Cluster::new(2);
    let t: Value = c[0]
        .invoke("todo_task_create", json!({ "title": "Plan the hike" }))
        .unwrap();
    let id = t["id"].clone();
    c.sync(0, 1);
    let task = |app: &TestApp| -> Value { app.invoke("todo_task", json!({ "id": id })).unwrap() };

    hold(&c[1], TASK, &id, "title");
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>("todo_task_set_priority", json!({ "id": id, "priority": 1 }))
        .unwrap();
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>(
        "todo_task_update",
        json!({ "id": id, "title": "Plan the long hike", "notes": "Boots", "sessionId": "s-a" }),
    )
    .unwrap();
    c.sync(0, 1);
    let there = task(&c[1]);
    assert_eq!(there["priority"], 1, "other fields refresh");
    assert_eq!(there["notes"], "Boots", "the notes are not being typed in");
    assert_eq!(there["title"], "Plan the hike", "the title waits");

    release(&c[1], TASK, &id, "title");
    assert_eq!(task(&c[1])["title"], "Plan the long hike");
    c.sync(0, 1);
    assert_eq!(task(&c[0])["title"], task(&c[1])["title"]);
}

#[test]
fn a_task_field_held_and_then_saved_locally_keeps_the_local_text_everywhere() {
    let c = Cluster::new(2);
    let t: Value = c[0]
        .invoke("todo_task_create", json!({ "title": "Call mum" }))
        .unwrap();
    let id = t["id"].clone();
    c.sync(0, 1);
    hold(&c[1], TASK, &id, "notes");
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>(
        "todo_task_update",
        json!({ "id": id, "title": "Call mum", "notes": "from A", "sessionId": "s-a" }),
    )
    .unwrap();
    c.sync(0, 1);
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>(
        "todo_task_update",
        json!({ "id": id, "title": "Call mum", "notes": "typed on B", "sessionId": "s-b" }),
    )
    .unwrap();
    release(&c[1], TASK, &id, "notes");
    c.sync(0, 1);
    for d in 0..2 {
        let t: Value = c[d].invoke("todo_task", json!({ "id": id })).unwrap();
        assert_eq!(t["notes"], "typed on B", "device {d}");
    }
}

// ---------------------------------------------------------------- Undo

#[test]
fn undoing_a_delete_after_a_remote_edit_keeps_the_remote_edit() {
    let (c, id) = pair_with_entry("original");
    c[0].clock().advance(1_000);
    delete(&c[0], &id);
    c[1].clock().advance(2_000);
    edit(&c[1], &id, "edited elsewhere", "s-b");
    c.sync(0, 1);
    // Undo on A restores what A deleted, not the text A last saw.
    c[0].clock().advance(3_000);
    restore(&c[0], &id);
    for d in 0..2 {
        c.sync(0, 1);
        assert_eq!(
            body(&c[d], &id).as_deref(),
            Some("edited elsewhere"),
            "device {d}"
        );
    }
}

#[test]
fn undoing_a_move_after_a_remote_edit_puts_back_the_date_and_keeps_the_remote_text() {
    let (c, id) = pair_with_entry("original");
    c[0].clock().advance(1_000);
    move_to(&c[0], &id, "2026-10-28");
    c[1].clock().advance(2_000);
    edit(&c[1], &id, "edited elsewhere", "s-b");
    c.sync(0, 1);
    c[0].clock().advance(3_000);
    move_to(&c[0], &id, "2026-10-20");
    c.sync(0, 1);
    for d in 0..2 {
        assert_eq!(
            shown(&c[d]),
            [("2026-10-20".into(), "edited elsewhere".into())],
            "device {d}"
        );
    }
}

#[test]
fn undoing_a_task_change_after_a_remote_change_to_the_same_task_keeps_the_remote_change() {
    let c = Cluster::new(2);
    let t: Value = c[0]
        .invoke("todo_task_create", json!({ "title": "Pay rent" }))
        .unwrap();
    let id = t["id"].clone();
    c.sync(0, 1);
    let get = |app: &TestApp| -> Value { app.invoke("todo_task", json!({ "id": id })).unwrap() };
    // A raises the priority (a toast offers Undo); B changes the title meanwhile.
    c[0].clock().advance(1_000);
    c[0].invoke::<Value>("todo_task_set_priority", json!({ "id": id, "priority": 1 }))
        .unwrap();
    c[1].clock().advance(2_000);
    c[1].invoke::<Value>(
        "todo_task_update",
        json!({ "id": id, "title": "Pay the rent", "notes": "", "sessionId": "s-b" }),
    )
    .unwrap();
    c.sync(0, 1);
    // Undo: the Priority goes back to what it was, and only that.
    c[0].clock().advance(3_000);
    c[0].invoke::<Value>("todo_task_set_priority", json!({ "id": id, "priority": 4 }))
        .unwrap();
    c.sync(0, 1);
    for d in 0..2 {
        let t = get(&c[d]);
        assert_eq!(
            t["title"], "Pay the rent",
            "device {d} keeps the remote change"
        );
        assert_eq!(t["priority"], 4, "device {d}");
    }
}
