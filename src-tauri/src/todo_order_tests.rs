//! Command-layer tests for ordering, Group by and sorts that the shared fixtures cannot reach:
//! what the change log holds, History, the Done-in-Lists Preference (which needs closing times),
//! Preferences the Settings would refuse, and the key migration. Every Group by x sort
//! combination is in `fixtures/todo-order-rules.json`. They drive the real commands (ADR 0009).

use mnemora_core::db::Connection;
use serde_json::{Value, json};

use crate::test_support::{TestApp, tasks_of};

const TODAY: &str = "2026-10-21";
const DAY_MS: i64 = 86_400_000;

fn local() -> Value {
    json!({ "today": TODAY, "now": "14:05", "offset_min": 0 })
}

fn create(app: &TestApp, title: &str, list: &Value) -> Value {
    app.invoke(
        "todo_task_create",
        json!({ "title": title, "listId": list }),
    )
    .unwrap()
}

fn new_list(app: &TestApp, name: &str) -> Value {
    app.invoke("todo_list_create", json!({ "name": name }))
        .unwrap()
}

fn reorder(app: &TestApp, task: &Value, before: &Value, after: &Value, target: &Value) -> Value {
    app.invoke(
        "todo_task_reorder",
        json!({
            "id": task["id"], "before": before, "after": after, "target": target, "local": local()
        }),
    )
    .unwrap()
}

/// `(op, entity)` of every Todo change from the n-th on, oldest first.
fn ops_since(app: &TestApp, skip: usize) -> Vec<(String, String)> {
    app.with_db(|c| {
        let mut stmt = c
            .prepare("SELECT op, entity FROM change_log WHERE module = 'todo' ORDER BY at, rowid")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .skip(skip)
            .map(Result::unwrap)
            .collect()
    })
}

fn pair(op: &str, entity: &str) -> (String, String) {
    (op.to_owned(), entity.to_owned())
}

fn history(app: &TestApp, task: &Value) -> Vec<String> {
    let lines: Value = app
        .invoke("todo_task_history", json!({ "id": task["id"] }))
        .unwrap();
    lines
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["text"].as_str().unwrap().to_owned())
        .collect()
}

// ----------------------------------------------------------------------- the change log

#[test]
fn a_reorder_writes_one_row_and_one_change_per_move() {
    let app = TestApp::new();
    let tasks: Vec<Value> = ["a", "b", "c"]
        .iter()
        .map(|t| create(&app, t, &Value::Null))
        .collect();
    let keys = |app: &TestApp| -> Vec<String> {
        app.with_db(|c| {
            let mut stmt = c
                .prepare("SELECT sort_key FROM todo_tasks ORDER BY title")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        })
    };
    let (before_keys, logged) = (keys(&app), ops_since(&app, 0).len());
    let moved = reorder(
        &app,
        &tasks[2],
        &tasks[0]["id"],
        &tasks[1]["id"],
        &json!([]),
    );
    let after_keys = keys(&app);
    assert_eq!(
        before_keys[..2],
        after_keys[..2],
        "the neighbours are untouched"
    );
    assert_ne!(before_keys[2], after_keys[2]);
    assert_eq!(moved["sort_key"], after_keys[2]);
    assert_eq!(ops_since(&app, logged), [pair("update", "todo_task")]);
}

#[test]
fn a_reorder_into_a_list_group_is_a_move_and_the_snapshot_never_holds_text() {
    let app = TestApp::new();
    let home = new_list(&app, "Home");
    let a = create(&app, "secret title", &Value::Null);
    let logged = ops_since(&app, 0).len();
    reorder(
        &app,
        &a,
        &Value::Null,
        &Value::Null,
        &json!([{ "kind": "list", "list_id": home["id"] }]),
    );
    assert_eq!(ops_since(&app, logged), [pair("move", "todo_task")]);
    let lines = history(&app, &a);
    assert_eq!(lines[0], "Moved 📥 Inbox → ≡ Home");
}

#[test]
fn a_plain_reorder_adds_no_history_line_and_a_date_drop_does() {
    let app = TestApp::new();
    let a = create(&app, "a", &Value::Null);
    let b = create(&app, "b", &Value::Null);
    reorder(&app, &b, &Value::Null, &a["id"], &json!([]));
    assert_eq!(history(&app, &b), ["Created"]);
    reorder(
        &app,
        &b,
        &Value::Null,
        &Value::Null,
        &json!([{ "kind": "day", "date": "2026-10-23" }, { "kind": "priority", "priority": 1 }]),
    );
    let lines = history(&app, &b);
    assert!(lines.contains(&"Priority P4 → P1".to_owned()), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("Due date")), "{lines:?}");
}

#[test]
fn a_refused_reorder_changes_and_logs_nothing() {
    let app = TestApp::new();
    let a = create(&app, "a", &Value::Null);
    let logged = ops_since(&app, 0).len();
    let err = app
        .invoke::<Value>(
            "todo_task_reorder",
            json!({
                "id": a["id"], "before": null, "after": null,
                "target": [{ "kind": "priority", "priority": 2 }, { "kind": "day", "date": "2026-10-20" }],
                "local": local()
            }),
        )
        .unwrap_err();
    assert_eq!(err["code"], "validation");
    assert_eq!(ops_since(&app, logged), []);
    let still: Value = app.invoke("todo_task", json!({ "id": a["id"] })).unwrap();
    assert_eq!(still["priority"], 4, "the Priority was not half-applied");
}

#[test]
fn list_changes_are_recorded_as_updates() {
    let app = TestApp::new();
    let (a, b) = (new_list(&app, "a"), new_list(&app, "b"));
    let logged = ops_since(&app, 0).len();
    app.invoke::<Value>(
        "todo_list_reorder",
        json!({ "id": b["id"], "before": null, "after": a["id"] }),
    )
    .unwrap();
    app.invoke::<Value>(
        "todo_list_set_sort",
        json!({ "id": a["id"], "sort": "title" }),
    )
    .unwrap();
    app.invoke::<Value>(
        "todo_list_set_sort",
        json!({ "id": a["id"], "sort": "title" }),
    )
    .unwrap();
    app.invoke::<Value>(
        "todo_list_set_group",
        json!({ "id": a["id"], "groupBy": "priority" }),
    )
    .unwrap();
    assert_eq!(
        ops_since(&app, logged),
        [
            pair("update", "todo_list"),
            pair("update", "todo_list"),
            pair("update", "todo_list")
        ],
        "setting the same sort again records nothing"
    );
    let sidebar: Value = app
        .invoke("todo_lists", json!({ "local": local() }))
        .unwrap();
    assert_eq!(sidebar["lists"][0]["list"]["name"], "b");
    assert_eq!(sidebar["lists"][1]["list"]["task_sort"], "title");
    assert_eq!(sidebar["lists"][1]["list"]["group_by"], "priority");
    assert_eq!(sidebar["lists_sort"], "manual");
}

// ----------------------------------------------------------------------- Done in Lists

/// A Home List with one Task done today, one yesterday, one a week ago and one Won't do today.
fn home_with_done(app: &TestApp) -> Value {
    let home = new_list(app, "Home");
    let open = create(app, "open", &home["id"]);
    let _ = open;
    let (start, _) = (
        // 2026-10-21T00:00Z in UTC milliseconds.
        20_747 * DAY_MS,
        0,
    );
    for (title, closed_at, state) in [
        ("done today", start + 3_600_000, "done"),
        ("done yesterday", start - 3_600_000, "done"),
        ("done last week", start - 7 * DAY_MS, "done"),
        ("wont today", start + 7_200_000, "wont"),
    ] {
        let t = create(app, title, &home["id"]);
        app.with_db(|c: &Connection| {
            c.execute(
                &format!(
                    "UPDATE todo_tasks SET state = '{state}', closed_at = {closed_at} WHERE id = '{}'",
                    t["id"].as_str().unwrap()
                ),
                [],
            )
            .unwrap();
        });
    }
    home
}

fn done_titles(view: &Value, part: &str) -> Vec<String> {
    view["done"][part]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn done_tasks_in_a_list_follow_the_preference() {
    let app = TestApp::new();
    let home = home_with_done(&app);
    let list = |app: &TestApp| -> Value {
        app.invoke("todo_list", json!({ "id": home["id"], "local": local() }))
            .unwrap()
    };

    // The default: done today (and Won't do today) shown, newest first; older behind a fold.
    let view = list(&app);
    assert_eq!(view["done"]["mode"], "today_shown");
    assert_eq!(done_titles(&view, "shown"), ["wont today", "done today"]);
    assert_eq!(
        done_titles(&view, "folded"),
        ["done yesterday", "done last week"]
    );
    assert_eq!(titles_of(&view), ["open"], "done Tasks are not open Tasks");

    app.set_pref("todo.done_in_lists", json!("folded"));
    let view = list(&app);
    assert!(done_titles(&view, "shown").is_empty());
    assert_eq!(done_titles(&view, "folded").len(), 4);

    app.set_pref("todo.done_in_lists", json!("shown"));
    let view = list(&app);
    assert_eq!(done_titles(&view, "shown").len(), 4);
    assert!(done_titles(&view, "folded").is_empty());

    app.set_pref("todo.done_in_lists", json!("hidden"));
    let view = list(&app);
    assert_eq!(view["done"]["mode"], "hidden");
    assert!(done_titles(&view, "shown").is_empty() && done_titles(&view, "folded").is_empty());
}

#[test]
fn the_done_day_is_the_devices_day() {
    let app = TestApp::new();
    let home = home_with_done(&app);
    // Kolkata is 5h30 ahead: 2026-10-21 starts at 18:30Z the evening before, so "done yesterday"
    // (23:00Z on the 20th) is today there.
    let view: Value = app
        .invoke(
            "todo_list",
            json!({
                "id": home["id"],
                "local": { "today": TODAY, "now": "14:05", "offset_min": 330 }
            }),
        )
        .unwrap();
    assert_eq!(
        done_titles(&view, "shown"),
        ["wont today", "done today", "done yesterday"]
    );
}

fn titles_of(view: &Value) -> Vec<String> {
    tasks_of(view)
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_owned())
        .collect()
}

// ----------------------------------------------------------------------- Preferences

#[test]
fn the_settings_refuse_a_grouping_a_view_does_not_offer() {
    let app = TestApp::new();
    for (key, bad) in [
        ("todo.group_today", "due"),
        ("todo.group_upcoming", "none"),
        ("todo.group_inbox", "list"),
        ("todo.group_all", "day"),
        ("todo.inbox_sort", "name_asc"),
        ("todo.lists_sort", "title"),
        ("todo.done_in_lists", "all"),
    ] {
        let err = app
            .invoke::<Value>(
                "settings_save",
                json!({ "changes": [{ "key": key, "value": bad }] }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "validation", "{key} = {bad}");
    }
    let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
    let defaults: Vec<_> = [
        "todo.group_today",
        "todo.group_upcoming",
        "todo.group_inbox",
        "todo.group_all",
        "todo.inbox_sort",
        "todo.lists_sort",
        "todo.done_in_lists",
    ]
    .iter()
    .map(|k| snapshot["values"][k].clone())
    .collect();
    assert_eq!(
        defaults,
        [
            "priority",
            "day",
            "none",
            "list",
            "manual",
            "manual",
            "today_shown"
        ]
    );
}

#[test]
fn a_saved_value_that_is_no_longer_valid_reads_as_the_default() {
    let app = TestApp::new();
    create(&app, "a", &Value::Null);
    app.with_db(|c| {
        c.execute(
            "INSERT INTO settings(module, key, value, updated_at) VALUES ('todo', 'group_inbox', '\"bogus\"', 0)",
            [],
        )
        .unwrap();
    });
    let view: Value = app
        .invoke("todo_inbox", json!({ "local": local() }))
        .unwrap();
    assert_eq!(view["group_by"], "none");
}

// ----------------------------------------------------------------------- key migration

#[test]
fn keys_from_the_first_slices_lose_their_trailing_zeros_and_keep_their_order() {
    use mnemora_core::module::Module;
    use mnemora_todo::TodoModule;
    let migrations = TodoModule.migrations();
    let mut conn = Connection::open_in_memory().unwrap();
    migrations.to_version(&mut conn, 2).unwrap();
    let keys = ["V0000000", "V0000001", "V000000z", "V0000010", "W0000000"];
    for (i, key) in keys.iter().enumerate() {
        conn.execute(
            &format!(
                "INSERT INTO todo_tasks(id, title, sort_key, created_at, updated_at)
                 VALUES ('t{i}', 't', '{key}', 0, 0)"
            ),
            [],
        )
        .unwrap();
        conn.execute(
            &format!(
                "INSERT INTO todo_lists(id, name, name_key, sort_key, created_at, updated_at)
                 VALUES ('l{i}', 'l{i}', 'l{i}', '{key}', 0, 0)"
            ),
            [],
        )
        .unwrap();
    }
    migrations.to_latest(&mut conn).unwrap();
    for table in ["todo_tasks", "todo_lists"] {
        let rows: Vec<String> = conn
            .prepare(&format!("SELECT sort_key FROM {table} ORDER BY sort_key"))
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            rows,
            ["V", "V0000001", "V000000z", "V000001", "W"],
            "{table}: the same order, no key ends in 0"
        );
    }
}
