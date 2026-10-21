//! Command-layer tests for capture: `todo_parse` against the live Lists, a Task created with
//! what capture read, Quick edit and its Undo, and the Preference that turns recognition off. They
//! drive the real commands through the mock runtime (ADR 0009).

use serde_json::{Value, json};

use crate::test_support::{TestApp, tasks_of};

const TODAY: &str = "2026-10-21";

fn local() -> Value {
    json!({ "today": TODAY, "now": "14:05", "offset_min": 0 })
}

fn parse(app: &TestApp, text: &str) -> Value {
    app.invoke(
        "todo_parse",
        json!({ "request": {
            "text": text, "ignored": [], "prefer": {}, "frozen": [],
            "today": TODAY, "now": "14:05", "week_start": "mon",
        } }),
    )
    .unwrap()
}

fn list(app: &TestApp, name: &str) -> Value {
    app.invoke("todo_list_create", json!({ "name": name }))
        .unwrap()
}

fn create(
    app: &TestApp,
    title: &str,
    details: Value,
    when: Value,
    list_id: Value,
) -> Result<Value, Value> {
    app.invoke(
        "todo_task_create",
        json!({ "title": title, "listId": list_id, "when": when, "details": details }),
    )
}

fn get(app: &TestApp, task: &Value) -> Value {
    app.invoke("todo_task", json!({ "id": task["id"] }))
        .unwrap()
}

fn log_len(app: &TestApp) -> i64 {
    app.with_db(|c| {
        c.query_row(
            "SELECT COUNT(*) FROM change_log WHERE module = 'todo'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    })
}

fn edit_of(task: &Value) -> Value {
    json!({
        "title": task["title"], "understood": task["understood"],
        "due_date": task["due_date"], "due_time": task["due_time"],
        "repeat": null, "priority": task["priority"], "list_id": task["list_id"],
    })
}

fn quick(app: &TestApp, task: &Value, edit: &Value) -> Result<Value, Value> {
    app.invoke(
        "todo_task_quick_edit",
        json!({ "id": task["id"], "edit": edit, "local": local() }),
    )
}

// ------------------------------------------------------------------ parse

#[test]
fn parse_matches_hash_names_against_the_live_lists() {
    let app = TestApp::new();
    let home = list(&app, "Home");
    let parsed = parse(&app, "Fix tap #ho tomorrow");
    assert_eq!(parsed["list_id"], home["id"]);
    assert_eq!(parsed["date"], "2026-10-22");
    assert_eq!(parsed["new_list"], Value::Null);
    let spans = parsed["spans"].as_array().unwrap();
    assert_eq!(spans.len(), 2);
    assert_eq!(
        (spans[0]["word"].as_str(), spans[0]["kind"].as_str()),
        (Some("#ho"), Some("list"))
    );
}

#[test]
fn parse_offers_a_new_list_for_an_unknown_name_and_creates_nothing() {
    let app = TestApp::new();
    let parsed = parse(&app, "Buy paint #Garage");
    assert_eq!(parsed["new_list"], "Garage");
    assert_eq!(parsed["list_id"], Value::Null);
    let sidebar: Value = app
        .invoke("todo_lists", json!({ "local": local() }))
        .unwrap();
    assert!(sidebar["lists"].as_array().unwrap().is_empty());
}

#[test]
fn parse_ignores_deleted_lists_and_writes_nothing() {
    let app = TestApp::new();
    let gone = list(&app, "Garden");
    app.invoke::<Value>(
        "todo_list_delete",
        json!({ "id": gone["id"], "choice": { "kind": "delete_tasks" } }),
    )
    .unwrap();
    let before = log_len(&app);
    let parsed = parse(&app, "Water #gar");
    assert_eq!(parsed["list_id"], Value::Null);
    assert_eq!(log_len(&app), before);
}

#[test]
fn parse_honours_ignored_prefer_and_frozen_words() {
    let app = TestApp::new();
    let parsed: Value = app
        .invoke(
            "todo_parse",
            json!({ "request": {
                "text": "Pay rent tomorrow to tell today", "ignored": ["date:today:1"],
                "prefer": {}, "frozen": [], "today": TODAY, "now": "14:05", "week_start": "mon",
            } }),
        )
        .unwrap();
    assert_eq!(parsed["date"], "2026-10-22");
    assert_eq!(parsed["ignored"][0]["word"], "today");
}

// ----------------------------------------------------------------- create

#[test]
fn a_captured_task_keeps_its_notes_priority_repeat_and_understood_words() {
    let app = TestApp::new();
    let understood = r#"[{"word":"tomorrow","kind":"date","value":"2026-10-22"}]"#;
    let task = create(
        &app,
        "Pay rent tomorrow every month",
        json!({
            "notes": "- [ ] transfer", "priority": 1, "understood": understood,
            "repeat": { "freq": "month", "every": 1, "days": null, "mode": "fixed" },
        }),
        json!({ "date": "2026-10-22", "time": "09:00", "local": local() }),
        Value::Null,
    )
    .unwrap();
    assert_eq!(task["title"], "Pay rent tomorrow every month");
    assert_eq!(task["notes"], "- [ ] transfer");
    assert_eq!(task["priority"], 1);
    assert_eq!(task["due_date"], "2026-10-22");
    assert_eq!(task["due_time"], "09:00");
    assert_eq!(task["repeat_freq"], "month");
    assert_eq!(task["repeat_days"], 22);
    assert_eq!(task["understood"], understood);
    assert_eq!(get(&app, &task)["understood"], understood);
}

#[test]
fn a_repeat_without_a_date_lands_on_its_first_day() {
    let app = TestApp::new();
    let task = create(
        &app,
        "Gym",
        json!({ "repeat": { "freq": "week", "every": 1, "days": 18, "mode": "fixed" } }),
        json!({ "date": null, "time": null, "local": local() }),
        Value::Null,
    )
    .unwrap();
    assert_eq!(task["due_date"], "2026-10-22");
}

#[test]
fn capture_details_are_validated() {
    let app = TestApp::new();
    let when = json!({ "date": null, "time": null, "local": local() });
    for (details, why) in [
        (json!({ "priority": 5 }), "priority"),
        (json!({ "understood": "not json" }), "understood"),
        (
            json!({ "understood": "{\"a\":1}" }),
            "understood is an array",
        ),
        (
            json!({ "repeat": { "freq": "week", "every": 1, "days": null, "mode": "fixed" } }),
            "repeat needs a date",
        ),
    ] {
        let when = if why == "repeat needs a date" {
            Value::Null
        } else {
            when.clone()
        };
        let err = create(&app, "x", details, when, Value::Null).unwrap_err();
        assert_eq!(err["code"], "validation", "{why}");
    }
    let err = create(
        &app,
        "x",
        json!({ "notes": "n".repeat(100_001) }),
        Value::Null,
        Value::Null,
    )
    .unwrap_err();
    assert_eq!(err["code"], "validation");
    let inbox: Value = app
        .invoke(
            "todo_inbox",
            json!({ "local": { "today": "2026-10-21", "now": "14:05", "offset_min": 0 } }),
        )
        .unwrap();
    assert!(tasks_of(&inbox).as_array().unwrap().is_empty());
}

#[test]
fn a_created_task_is_one_create_record() {
    let app = TestApp::new();
    let before = log_len(&app);
    create(
        &app,
        "x",
        json!({ "priority": 2 }),
        json!({ "date": "2026-10-23", "time": null, "local": local() }),
        Value::Null,
    )
    .unwrap();
    assert_eq!(log_len(&app), before + 1);
}

// ------------------------------------------------------------- quick edit

#[test]
fn quick_edit_saves_every_field_in_one_update() {
    let app = TestApp::new();
    let work = list(&app, "Work");
    let task = create(&app, "Pay rent", json!({}), Value::Null, Value::Null).unwrap();
    let before = log_len(&app);
    let mut edit = edit_of(&task);
    edit["title"] = json!("Pay rent tomorrow p1 #work");
    edit["due_date"] = json!("2026-10-22");
    edit["priority"] = json!(1);
    edit["list_id"] = work["id"].clone();
    edit["understood"] = json!(r#"[{"word":"tomorrow","kind":"date","value":"2026-10-22"}]"#);
    let saved = quick(&app, &task, &edit).unwrap();
    assert_eq!(saved["title"], "Pay rent tomorrow p1 #work");
    assert_eq!(saved["due_date"], "2026-10-22");
    assert_eq!(saved["priority"], 1);
    assert_eq!(saved["list_id"], work["id"]);
    assert_eq!(get(&app, &task)["understood"], edit["understood"]);
    assert_eq!(log_len(&app), before + 1);
}

#[test]
fn quick_edit_with_nothing_changed_records_nothing() {
    let app = TestApp::new();
    let task = create(&app, "Pay rent", json!({}), Value::Null, Value::Null).unwrap();
    let before = log_len(&app);
    quick(&app, &task, &edit_of(&task)).unwrap();
    assert_eq!(log_len(&app), before);
}

#[test]
fn quick_edit_refuses_a_past_date_but_lets_an_overdue_task_keep_its_own() {
    let app = TestApp::new();
    let task = create(&app, "Old", json!({}), Value::Null, Value::Null).unwrap();
    let mut edit = edit_of(&task);
    edit["due_date"] = json!("2026-10-20");
    assert_eq!(quick(&app, &task, &edit).unwrap_err()["code"], "validation");

    // Undo may put a past date back; an edit that leaves the date alone may keep it.
    let undo = |t: &Value, e: &Value| {
        app.invoke::<Value>(
            "todo_task_quick_edit_undo",
            json!({ "id": t["id"], "edit": e, "local": local() }),
        )
        .unwrap()
    };
    let overdue = undo(&task, &edit);
    assert_eq!(overdue["due_date"], "2026-10-20");
    let mut retitle = edit_of(&overdue);
    retitle["title"] = json!("Old, renamed");
    let saved = quick(&app, &overdue, &retitle).unwrap();
    assert_eq!(saved["title"], "Old, renamed");
    assert_eq!(saved["due_date"], "2026-10-20");
}

#[test]
fn undo_puts_the_old_values_back() {
    let app = TestApp::new();
    let task = create(
        &app,
        "Pay rent",
        json!({ "priority": 3 }),
        Value::Null,
        Value::Null,
    )
    .unwrap();
    let old = edit_of(&task);
    let mut edit = old.clone();
    edit["title"] = json!("Pay rent today");
    edit["due_date"] = json!("2026-10-21");
    edit["priority"] = json!(1);
    quick(&app, &task, &edit).unwrap();
    let back: Value = app
        .invoke(
            "todo_task_quick_edit_undo",
            json!({ "id": task["id"], "edit": old, "local": local() }),
        )
        .unwrap();
    assert_eq!(back["title"], "Pay rent");
    assert_eq!(back["due_date"], Value::Null);
    assert_eq!(back["priority"], 3);
}

#[test]
fn quick_edit_validates_and_needs_an_open_task() {
    let app = TestApp::new();
    let task = create(&app, "Pay rent", json!({}), Value::Null, Value::Null).unwrap();
    for (field, bad) in [
        ("title", json!("  ")),
        ("title", json!("two\nlines")),
        ("priority", json!(7)),
        ("list_id", json!("nope")),
        ("understood", json!("nope")),
    ] {
        let mut edit = edit_of(&task);
        edit[field] = bad;
        assert!(quick(&app, &task, &edit).is_err(), "{field}");
    }
    app.invoke::<Value>(
        "todo_task_complete",
        json!({ "id": task["id"], "local": local() }),
    )
    .unwrap();
    let mut edit = edit_of(&task);
    edit["priority"] = json!(1);
    assert_eq!(quick(&app, &task, &edit).unwrap_err()["code"], "validation");
    // The title of a done Task can still be fixed.
    let mut retitle = edit_of(&get(&app, &task));
    retitle["title"] = json!("Paid rent");
    assert_eq!(quick(&app, &task, &retitle).unwrap()["title"], "Paid rent");
}

#[test]
fn quick_edit_on_a_repeating_task_keeps_its_repeat_when_the_date_moves() {
    let app = TestApp::new();
    let task = create(
        &app,
        "Pay rent",
        json!({ "repeat": { "freq": "month", "every": 1, "days": null, "mode": "fixed" } }),
        json!({ "date": "2026-10-25", "time": null, "local": local() }),
        Value::Null,
    )
    .unwrap();
    let mut edit = edit_of(&task);
    edit["repeat"] = json!({ "freq": "month", "every": 1, "days": 25, "mode": "fixed" });
    edit["due_date"] = json!("2026-10-28");
    let saved = quick(&app, &task, &edit).unwrap();
    assert_eq!(saved["due_date"], "2026-10-28");
    assert_eq!(saved["repeat_freq"], "month");
    assert_eq!(saved["repeat_days"], 28);
}

// ------------------------------------------------------------- Preference

#[test]
fn recognising_dates_is_on_by_default_and_takes_only_a_boolean() {
    let app = TestApp::new();
    let snap: Value = app.invoke("settings_get_all", json!({})).unwrap();
    assert_eq!(snap["values"]["todo.parse_dates"], true);
    let save = |v: Value| {
        app.invoke::<()>(
            "settings_save",
            json!({ "changes": [{ "key": "todo.parse_dates", "value": v }] }),
        )
    };
    save(json!(false)).unwrap();
    let snap: Value = app.invoke("settings_get_all", json!({})).unwrap();
    assert_eq!(snap["values"]["todo.parse_dates"], false);
    assert_eq!(save(json!("off")).unwrap_err()["code"], "validation");
}
