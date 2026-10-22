//! Command-layer tests for the Task sheet's rules: Priority, Won't do, open steps, delete and
//! purge, Comments and History. They drive the real commands through the mock runtime (ADR 0009).

use mnemora_todo::TodoModule;
use serde_json::{Value, json};

use crate::test_support::{TestApp, close_all_on, inbox_tasks_of, tasks_of};

const TODAY: &str = "2026-10-21";

fn local(today: &str) -> Value {
    json!({ "today": today, "now": "14:05", "offset_min": 0 })
}

fn create(app: &TestApp, title: &str) -> Value {
    app.invoke("todo_task_create", json!({ "title": title }))
        .unwrap()
}

fn dated(app: &TestApp, title: &str, date: &str, time: Option<&str>) -> Value {
    let task = create(app, title);
    app.invoke(
        "todo_task_set_when",
        json!({ "id": task["id"], "date": date, "time": time, "local": local(TODAY) }),
    )
    .unwrap()
}

fn priority(app: &TestApp, task: &Value, p: u32) -> Result<Value, Value> {
    app.invoke(
        "todo_task_set_priority",
        json!({ "id": task["id"], "priority": p }),
    )
}

fn get(app: &TestApp, task: &Value) -> Value {
    app.invoke("todo_task", json!({ "id": task["id"] }))
        .unwrap()
}

fn titles(tasks: &Value) -> Vec<String> {
    tasks
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_owned())
        .collect()
}

fn inbox(app: &TestApp) -> Vec<String> {
    let view: Value = app
        .invoke("todo_inbox", json!({ "local": local(TODAY) }))
        .unwrap();
    titles(&inbox_tasks_of(&view))
}

fn complete(app: &TestApp, task: &Value) -> Result<Value, Value> {
    app.invoke(
        "todo_task_complete",
        json!({ "id": task["id"], "local": local(TODAY) }),
    )
}

fn update(app: &TestApp, task: &Value, title: &str, notes: &str, session: &str) -> Value {
    app.invoke(
        "todo_task_update",
        json!({ "id": task["id"], "title": title, "notes": notes, "sessionId": session }),
    )
    .unwrap()
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

/// `(op, entity)` of every Todo change, oldest first.
fn log(app: &TestApp) -> Vec<(String, String)> {
    app.with_db(|c| {
        let mut stmt = c
            .prepare("SELECT op, entity FROM change_log WHERE module = 'todo' ORDER BY at, rowid")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    })
}

fn log_len(app: &TestApp) -> usize {
    log(app).len()
}

fn count(app: &TestApp, sql: &str) -> i64 {
    app.with_db(|c| c.query_row(sql, [], |r| r.get(0)).unwrap())
}

// ---------------------------------------------------------------- Priority

#[test]
fn a_task_starts_at_priority_four_and_takes_one_to_four() {
    let app = TestApp::new();
    let a = create(&app, "a");
    assert_eq!(a["priority"], 4);
    for p in [1, 2, 3, 4] {
        assert_eq!(priority(&app, &a, p).unwrap()["priority"], p);
    }
    for bad in [0, 5, 99] {
        assert_eq!(priority(&app, &a, bad).unwrap_err()["code"], "validation");
    }
    assert_eq!(get(&app, &a)["priority"], 4);
}

#[test]
fn a_priority_change_is_one_update_and_the_same_priority_records_nothing() {
    let app = TestApp::new();
    let a = create(&app, "a");
    let before = log_len(&app);
    priority(&app, &a, 1).unwrap();
    priority(&app, &a, 1).unwrap();
    assert_eq!(
        log(&app)[before..],
        [("update".to_owned(), "todo_task".to_owned())]
    );
}

#[test]
fn priority_needs_an_open_task_and_a_known_one() {
    let app = TestApp::new();
    let a = create(&app, "a");
    complete(&app, &a).unwrap();
    assert_eq!(priority(&app, &a, 1).unwrap_err()["code"], "validation");
    let nobody = json!({ "id": "nope" });
    assert_eq!(priority(&app, &nobody, 1).unwrap_err()["code"], "not_found");
}

#[test]
fn within_a_time_block_today_orders_by_priority_then_manual_order() {
    let app = TestApp::new();
    app.set_pref("todo.group_today", json!("none"));
    let plain = dated(&app, "plain", TODAY, None);
    let urgent = dated(&app, "urgent", TODAY, None);
    let timed_low = dated(&app, "timed low", TODAY, Some("09:00"));
    let timed_high = dated(&app, "timed high", TODAY, Some("09:00"));
    let later = dated(&app, "later", TODAY, Some("10:00"));
    let late_overdue = dated(&app, "later but p1", TODAY, Some("10:00"));
    priority(&app, &urgent, 1).unwrap();
    priority(&app, &timed_high, 2).unwrap();
    priority(&app, &late_overdue, 3).unwrap();
    let _ = (plain, timed_low, later);
    let view: Value = app
        .invoke("todo_today", json!({ "local": local(TODAY) }))
        .unwrap();
    assert_eq!(
        titles(&tasks_of(&view)),
        [
            "timed high",
            "timed low",
            "later but p1",
            "later",
            "urgent",
            "plain"
        ],
        "a timed Task keeps its place before untimed ones; priority only orders inside a block"
    );
}

#[test]
fn upcoming_days_order_by_priority_inside_each_block_too() {
    let app = TestApp::new();
    let first = dated(&app, "first", "2026-10-23", None);
    let second = dated(&app, "second", "2026-10-23", None);
    priority(&app, &second, 2).unwrap();
    let _ = first;
    let view: Value = app
        .invoke("todo_upcoming", json!({ "local": local(TODAY) }))
        .unwrap();
    assert_eq!(titles(&view["groups"][0]["tasks"]), ["second", "first"]);
}

// ---------------------------------------------------------------- Won't do

#[test]
fn wont_do_closes_the_task_into_done_without_counting_as_done() {
    let app = TestApp::new();
    let a = dated(&app, "a", TODAY, None);
    let b = dated(&app, "b", TODAY, None);
    let wont: Value = app
        .invoke(
            "todo_task_wont_do",
            json!({ "id": a["id"], "local": local(TODAY) }),
        )
        .unwrap();
    assert_eq!(wont["state"], "wont");
    assert!(wont["closed_at"].as_i64().is_some());
    complete(&app, &b).unwrap();
    close_all_on(&app, TODAY);

    let today: Value = app
        .invoke("todo_today", json!({ "local": local(TODAY) }))
        .unwrap();
    assert_eq!(titles(&tasks_of(&today)), Vec::<String>::new());
    assert_eq!(titles(&today["done"]), ["b"], "Won't do is not done");
    assert_eq!(today["total"], 1, "and not part of the progress");

    let sidebar: Value = app
        .invoke("todo_lists", json!({ "local": local(TODAY) }))
        .unwrap();
    assert_eq!(sidebar["today_count"], 0);
    assert_eq!(sidebar["inbox_count"], 0);

    let done: Value = app
        .invoke(
            "todo_done",
            json!({ "before": null, "days": 7, "local": local(TODAY) }),
        )
        .unwrap();
    let listed: Vec<(String, String)> = done["days"][0]["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["title"].as_str().unwrap().to_owned(),
                t["state"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(listed.contains(&("a".into(), "wont".into())), "{listed:?}");
    assert!(listed.contains(&("b".into(), "done".into())), "{listed:?}");
}

#[test]
fn wont_do_is_a_tick_that_reopening_undoes() {
    let app = TestApp::new();
    let a = create(&app, "a");
    let before = log_len(&app);
    app.invoke::<Value>(
        "todo_task_wont_do",
        json!({ "id": a["id"], "local": local(TODAY) }),
    )
    .unwrap();
    let back: Value = app
        .invoke("todo_task_reopen", json!({ "id": a["id"] }))
        .unwrap();
    assert_eq!(
        (back["state"].as_str(), back["closed_at"].clone()),
        (Some("open"), Value::Null)
    );
    assert_eq!(inbox(&app), ["a"]);
    assert_eq!(
        log(&app)[before..],
        [
            ("tick".to_owned(), "todo_task".to_owned()),
            ("tick".to_owned(), "todo_task".to_owned())
        ]
    );
}

#[test]
fn wont_do_is_refused_for_a_closed_and_an_unknown_task() {
    let app = TestApp::new();
    let a = create(&app, "a");
    complete(&app, &a).unwrap();
    let wont = |task: &Value| {
        app.invoke::<Value>(
            "todo_task_wont_do",
            json!({ "id": task["id"], "local": local(TODAY) }),
        )
    };
    assert_eq!(wont(&a).unwrap_err()["code"], "validation");
    assert_eq!(
        wont(&json!({ "id": "nope" })).unwrap_err()["code"],
        "not_found"
    );
}

#[test]
fn wont_do_on_a_repeating_task_skips_it_in_rust() {
    let app = TestApp::new();
    let r = dated(&app, "r", TODAY, None);
    app.invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": r["id"], "date": TODAY, "time": null, "local": local(TODAY),
                "repeat": { "freq": "day", "every": 3, "days": null, "mode": "fixed" } }),
    )
    .unwrap();
    let before = log_len(&app);
    let skipped: Value = app
        .invoke(
            "todo_task_wont_do",
            json!({ "id": r["id"], "local": local(TODAY) }),
        )
        .unwrap();
    assert_eq!(skipped["state"], "open");
    assert_eq!(skipped["due_date"], "2026-10-24");
    assert_eq!(get(&app, &r)["due_date"], "2026-10-24");
    assert_eq!(
        log(&app)[before..],
        [("tick".to_owned(), "todo_task".to_owned())],
        "the same record a skip writes"
    );
    assert_eq!(history(&app, &r)[0], "Skipped (Wed → Sat)");
}

// ---------------------------------------------------------------- Open steps

#[test]
fn completing_with_open_steps_is_still_allowed_and_tick_all_is_a_notes_update_first() {
    let app = TestApp::new();
    let a = create(&app, "Pack");
    update(
        &app,
        &a,
        "Pack",
        "- [x] tent\n- [ ] stove\n- [ ] food",
        "s1",
    );
    // "Complete": the Dialog's plain button changes nothing but the state.
    let plain = create(&app, "Plain");
    update(&app, &plain, "Plain", "- [ ] one", "s2");
    let done = complete(&app, &plain).unwrap();
    assert_eq!(done["task"]["state"], "done");
    assert_eq!(done["task"]["notes"], "- [ ] one");

    // "Tick all and complete": the notes are updated, then the Task is completed.
    update(
        &app,
        &a,
        "Pack",
        "- [x] tent\n- [x] stove\n- [x] food",
        "s3",
    );
    let done = complete(&app, &a).unwrap();
    assert_eq!(done["task"]["state"], "done");
    assert_eq!(done["task"]["notes"], "- [x] tent\n- [x] stove\n- [x] food");
}

// ---------------------------------------------------------------- Editing sessions

#[test]
fn saves_of_one_session_make_one_record_and_a_new_session_makes_another() {
    let app = TestApp::new();
    let a = create(&app, "a");
    let before = log_len(&app);
    update(&app, &a, "a", "n", "s1");
    update(&app, &a, "a", "no", "s1");
    update(&app, &a, "a", "not", "s1");
    assert_eq!(log_len(&app), before + 1);
    update(&app, &a, "a2", "not", "s2");
    assert_eq!(log_len(&app), before + 2);
    // Saving what is stored writes nothing.
    update(&app, &a, "a2", "not", "s3");
    assert_eq!(log_len(&app), before + 2);
    let stored = get(&app, &a);
    assert_eq!(
        (stored["title"].as_str(), stored["notes"].as_str()),
        (Some("a2"), Some("not"))
    );
}

#[test]
fn updates_refuse_bad_titles_missing_sessions_and_unknown_tasks() {
    let app = TestApp::new();
    let a = create(&app, "a");
    let before = log_len(&app);
    let attempt = |title: &str, session: &str, id: &Value| {
        app.invoke::<Value>(
            "todo_task_update",
            json!({ "id": id, "title": title, "notes": "", "sessionId": session }),
        )
        .unwrap_err()["code"]
            .clone()
    };
    assert_eq!(attempt("", "s", &a["id"]), "validation");
    assert_eq!(attempt("two\nlines", "s", &a["id"]), "validation");
    assert_eq!(attempt(&"x".repeat(501), "s", &a["id"]), "validation");
    assert_eq!(attempt("ok", " ", &a["id"]), "validation");
    assert_eq!(attempt("ok", "s", &json!("nope")), "not_found");
    assert_eq!(log_len(&app), before);
    assert_eq!(get(&app, &a)["title"], "a");
}

#[test]
fn notes_and_titles_are_searchable_after_an_update() {
    let app = TestApp::new();
    let a = create(&app, "a");
    update(&app, &a, "Renamed", "remember the zucchini", "s");
    let hits = count(
        &app,
        "SELECT COUNT(*) FROM todo_tasks_fts WHERE todo_tasks_fts MATCH 'zucchini'",
    );
    assert_eq!(hits, 1);
}

// ---------------------------------------------------------------- Delete, restore, purge

#[test]
fn deleting_leaves_every_view_at_once_and_restore_puts_the_task_back_in_place() {
    let app = TestApp::new();
    let a = create(&app, "a");
    let b = create(&app, "b");
    create(&app, "c");
    let before = log_len(&app);
    app.invoke::<()>("todo_task_delete", json!({ "id": b["id"] }))
        .unwrap();
    assert_eq!(inbox(&app), ["a", "c"]);
    assert_eq!(
        app.invoke::<Value>("todo_task", json!({ "id": b["id"] }))
            .unwrap_err()["code"],
        "not_found"
    );
    let back: Value = app
        .invoke("todo_task_restore", json!({ "id": b["id"] }))
        .unwrap();
    assert_eq!(back["title"], "b");
    assert_eq!(inbox(&app), ["a", "b", "c"]);
    assert_eq!(
        log(&app)[before..],
        [
            ("delete".to_owned(), "todo_task".to_owned()),
            ("restore".to_owned(), "todo_task".to_owned())
        ]
    );
    let _ = a;
    let deleted_at: Option<i64> = app.with_db(|c| {
        c.query_row(
            "SELECT json_extract(snapshot, '$.deleted_at') FROM change_log
             WHERE op = 'delete' AND entity = 'todo_task'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    });
    assert!(
        deleted_at.is_some(),
        "a delete record shows when it happened"
    );
}

#[test]
fn deleting_twice_restoring_a_live_task_and_unknown_ids_are_refused() {
    let app = TestApp::new();
    let a = create(&app, "a");
    let restore = |id: &Value| app.invoke::<Value>("todo_task_restore", json!({ "id": id }));
    assert_eq!(restore(&a["id"]).unwrap_err()["code"], "validation");
    app.invoke::<()>("todo_task_delete", json!({ "id": a["id"] }))
        .unwrap();
    let again = app.invoke::<()>("todo_task_delete", json!({ "id": a["id"] }));
    assert_eq!(again.unwrap_err()["code"], "not_found");
    assert_eq!(restore(&json!("nope")).unwrap_err()["code"], "not_found");
}

#[test]
fn a_task_restored_after_its_list_was_deleted_returns_to_the_inbox() {
    let app = TestApp::new();
    let list: Value = app
        .invoke("todo_list_create", json!({ "name": "Work" }))
        .unwrap();
    let a = create(&app, "a");
    app.invoke::<Value>(
        "todo_task_move",
        json!({ "id": a["id"], "listId": list["id"] }),
    )
    .unwrap();
    app.invoke::<()>("todo_task_delete", json!({ "id": a["id"] }))
        .unwrap();
    app.invoke::<Value>(
        "todo_list_delete",
        json!({ "id": list["id"], "choice": { "kind": "delete_tasks" } }),
    )
    .unwrap();
    let back: Value = app
        .invoke("todo_task_restore", json!({ "id": a["id"] }))
        .unwrap();
    assert_eq!(back["list_id"], Value::Null);
    assert_eq!(inbox(&app), ["a"]);
}

/// Pretends `days` days have passed since a row was deleted.
fn age(app: &TestApp, table: &str, id: &Value, days: i64) {
    app.with_db(|c| {
        c.execute(
            &format!(
                "UPDATE {table} SET deleted_at = CAST(strftime('%s', 'now') AS INTEGER) * 1000 - {} WHERE id = '{}'",
                days * 86_400_000,
                id.as_str().unwrap()
            ),
            [],
        )
        .unwrap();
    });
}

#[test]
fn what_was_deleted_over_thirty_days_ago_survives_a_restart_with_its_change_log() {
    let app = TestApp::new();
    let task = create(&app, "old");
    let list: Value = app
        .invoke("todo_list_create", json!({ "name": "Old" }))
        .unwrap();
    let comment = app
        .invoke::<Value>(
            "todo_comment_add",
            json!({ "taskId": create(&app, "host")["id"], "body": "gone" }),
        )
        .unwrap();
    app.invoke::<()>("todo_task_delete", json!({ "id": task["id"] }))
        .unwrap();
    app.invoke::<Value>(
        "todo_list_delete",
        json!({ "id": list["id"], "choice": { "kind": "move_to_inbox" } }),
    )
    .unwrap();
    app.invoke::<()>("todo_comment_delete", json!({ "id": comment["id"] }))
        .unwrap();
    age(&app, "todo_tasks", &task["id"], 31);
    age(&app, "todo_lists", &list["id"], 31);
    age(&app, "todo_comments", &comment["id"], 31);
    let log_before = log_len(&app);

    app.run_startup(&TodoModule);

    assert_eq!(count(&app, "SELECT COUNT(*) FROM todo_tasks"), 2);
    assert_eq!(count(&app, "SELECT COUNT(*) FROM todo_lists"), 1);
    assert_eq!(count(&app, "SELECT COUNT(*) FROM todo_comments"), 1);
    assert_eq!(log_len(&app), log_before, "no record is erased");
    let back: Value = app
        .invoke("todo_task_restore", json!({ "id": task["id"] }))
        .unwrap();
    assert_eq!(back["title"], "old", "the old deletion can still be undone");
}

// ---------------------------------------------------------------- Comments

fn comment(app: &TestApp, task: &Value, body: &str) -> Value {
    app.invoke(
        "todo_comment_add",
        json!({ "taskId": task["id"], "body": body }),
    )
    .unwrap()
}

fn bodies(app: &TestApp, task: &Value) -> Vec<String> {
    let all: Value = app
        .invoke("todo_comments", json!({ "taskId": task["id"] }))
        .unwrap();
    all.as_array()
        .unwrap()
        .iter()
        .map(|c| c["body"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn comments_are_added_oldest_first_trimmed_and_counted_on_the_task() {
    let app = TestApp::new();
    let a = create(&app, "a");
    assert_eq!(a["comment_count"], 0);
    let first = comment(&app, &a, "  first **mark**  ");
    comment(&app, &a, "second");
    assert_eq!(first["body"], "first **mark**");
    assert_eq!(first["edited"], false);
    assert_eq!(bodies(&app, &a), ["first **mark**", "second"]);
    assert_eq!(get(&app, &a)["comment_count"], 2);
    assert_eq!(
        app.invoke::<Value>("todo_inbox", json!({ "local": local(TODAY) }))
            .unwrap()["groups"][0]["tasks"][0]["comment_count"],
        2
    );
}

#[test]
fn bad_comments_are_refused_and_record_nothing() {
    let app = TestApp::new();
    let a = create(&app, "a");
    let before = log_len(&app);
    for bad in ["", "  ", "\n\n"] {
        let err = app
            .invoke::<Value>(
                "todo_comment_add",
                json!({ "taskId": a["id"], "body": bad }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "validation", "{bad:?}");
    }
    let long = "x".repeat(5001);
    let err = app
        .invoke::<Value>(
            "todo_comment_add",
            json!({ "taskId": a["id"], "body": long }),
        )
        .unwrap_err();
    assert_eq!(err["code"], "validation");
    let err = app
        .invoke::<Value>(
            "todo_comment_add",
            json!({ "taskId": "nope", "body": "hi" }),
        )
        .unwrap_err();
    assert_eq!(err["code"], "not_found");
    assert_eq!(log_len(&app), before);
}

#[test]
fn an_edited_comment_says_so_and_the_same_text_is_no_edit() {
    let app = TestApp::new();
    let a = create(&app, "a");
    let c = comment(&app, &a, "draft");
    let same: Value = app
        .invoke(
            "todo_comment_edit",
            json!({ "id": c["id"], "body": "draft" }),
        )
        .unwrap();
    assert_eq!(same["edited"], false);
    let edited: Value = app
        .invoke(
            "todo_comment_edit",
            json!({ "id": c["id"], "body": "final" }),
        )
        .unwrap();
    assert_eq!(
        (edited["body"].as_str(), edited["edited"].clone()),
        (Some("final"), json!(true))
    );
    let listed: Value = app
        .invoke("todo_comments", json!({ "taskId": a["id"] }))
        .unwrap();
    assert_eq!(listed[0]["edited"], true);
    let err = app
        .invoke::<Value>("todo_comment_edit", json!({ "id": "nope", "body": "x" }))
        .unwrap_err();
    assert_eq!(err["code"], "not_found");
}

#[test]
fn deleting_a_comment_is_undone_by_restoring_it_without_marking_it_edited() {
    let app = TestApp::new();
    let a = create(&app, "a");
    let c = comment(&app, &a, "keep me");
    app.invoke::<()>("todo_comment_delete", json!({ "id": c["id"] }))
        .unwrap();
    assert!(bodies(&app, &a).is_empty());
    assert_eq!(get(&app, &a)["comment_count"], 0);
    let back: Value = app
        .invoke("todo_comment_restore", json!({ "id": c["id"] }))
        .unwrap();
    assert_eq!(back["edited"], false);
    assert_eq!(bodies(&app, &a), ["keep me"]);
    let err = app
        .invoke::<Value>("todo_comment_restore", json!({ "id": c["id"] }))
        .unwrap_err();
    assert_eq!(err["code"], "validation");
    let ops: Vec<String> = log(&app)
        .into_iter()
        .filter(|(_, e)| e == "todo_comment")
        .map(|(op, _)| op)
        .collect();
    assert_eq!(ops, ["create", "delete", "restore"]);
}

#[test]
fn comments_move_with_the_occurrence_and_the_counts_follow() {
    let app = TestApp::new();
    let r = dated(&app, "r", TODAY, None);
    app.invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": r["id"], "date": TODAY, "time": null, "local": local(TODAY),
                "repeat": { "freq": "day", "every": 1, "days": null, "mode": "fixed" } }),
    )
    .unwrap();
    comment(&app, &r, "one");
    comment(&app, &r, "two");
    let done = complete(&app, &r).unwrap();
    assert_eq!(done["task"]["comment_count"], 0);
    assert_eq!(done["occurrence"]["comment_count"], 2);
    let copy = &done["occurrence"];
    assert_eq!(bodies(&app, copy), ["one", "two"]);
    assert!(bodies(&app, &r).is_empty());
    comment(&app, &r, "for the next one");
    let back: Value = app
        .invoke("todo_task_reopen", json!({ "id": copy["id"] }))
        .unwrap();
    assert_eq!(back["comment_count"], 3);
    assert_eq!(bodies(&app, &r), ["one", "two", "for the next one"]);
}

/// The Task's `updated_at` values of a Comment, from the table.
fn comment_updated_at(app: &TestApp, id: &Value) -> i64 {
    count(
        app,
        &format!(
            "SELECT updated_at FROM todo_comments WHERE id = '{}'",
            id.as_str().unwrap()
        ),
    )
}

/// Makes `r` a daily repeating Task due today.
fn repeating(app: &TestApp, title: &str) -> Value {
    let r = dated(app, title, TODAY, None);
    app.invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": r["id"], "date": TODAY, "time": null, "local": local(TODAY),
                "repeat": { "freq": "day", "every": 1, "days": null, "mode": "fixed" } }),
    )
    .unwrap()
}

#[test]
fn completing_the_same_occurrence_again_after_a_reopen_reuses_its_row() {
    let app = TestApp::new();
    let r = repeating(&app, "r");
    comment(&app, &r, "one");
    let first = complete(&app, &r).unwrap();
    let id = first["occurrence"]["id"].clone();
    app.invoke::<Value>("todo_task_reopen", json!({ "id": id }))
        .unwrap();
    let again = complete(&app, &r).unwrap();

    assert_eq!(again["occurrence"]["id"], id, "one occurrence, one id");
    assert_eq!(
        count(
            &app,
            "SELECT COUNT(*) FROM todo_tasks WHERE repeat_of IS NOT NULL"
        ),
        1
    );
    assert_eq!(
        count(
            &app,
            "SELECT COUNT(*) FROM todo_tasks WHERE deleted_at IS NOT NULL"
        ),
        0,
        "the deletion is cleared"
    );
    assert_eq!(again["occurrence"]["comment_count"], 1);
    assert_eq!(bodies(&app, &again["occurrence"]), ["one"]);
}

#[test]
fn deleting_a_comment_and_completing_with_comments_record_and_stamp_them() {
    let app = TestApp::new();
    let r = repeating(&app, "r");
    let gone = comment(&app, &r, "gone");
    let moved = comment(&app, &r, "moved");
    let comment_records = |app: &TestApp| {
        log(app)
            .into_iter()
            .filter(|(_, e)| e == "todo_comment")
            .map(|(op, _)| op)
            .collect::<Vec<_>>()
    };

    let before = comment_updated_at(&app, &gone["id"]);
    app.invoke::<()>("todo_comment_delete", json!({ "id": gone["id"] }))
        .unwrap();
    assert!(comment_updated_at(&app, &gone["id"]) > before);
    assert_eq!(comment_records(&app), ["create", "create", "delete"]);

    let before = comment_updated_at(&app, &moved["id"]);
    complete(&app, &r).unwrap();
    assert!(comment_updated_at(&app, &moved["id"]) > before);
    let deleted_one = comment_updated_at(&app, &gone["id"]);
    assert!(deleted_one > before - 1, "the deleted Comment moved too");
    // Both Comments moved (the deleted one too: it comes back with Undo), each recorded.
    assert_eq!(
        comment_records(&app),
        ["create", "create", "delete", "move", "move"]
    );
    let in_move: String = app.with_db(|c| {
        c.query_row(
            "SELECT snapshot FROM change_log WHERE op = 'move' ORDER BY rowid LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap()
    });
    assert!(
        in_move.contains("\"body\""),
        "a move carries the full state"
    );
}

#[test]
fn a_done_task_can_still_be_commented() {
    let app = TestApp::new();
    let a = create(&app, "a");
    complete(&app, &a).unwrap();
    comment(&app, &a, "afterthought");
    assert_eq!(get(&app, &a)["comment_count"], 1);
}

// ---------------------------------------------------------------- History

#[test]
fn history_reads_the_change_log_as_lines_newest_first() {
    let app = TestApp::new();
    let work: Value = app
        .invoke("todo_list_create", json!({ "name": "Work" }))
        .unwrap();
    app.invoke::<Value>(
        "todo_list_set_emoji",
        json!({ "id": work["id"], "emoji": "💼" }),
    )
    .unwrap();
    let a = create(&app, "Secret plan");
    app.invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": a["id"], "date": "2026-10-22", "time": null, "local": local(TODAY) }),
    )
    .unwrap();
    app.invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": a["id"], "date": "2026-10-23", "time": "09:30", "local": local(TODAY) }),
    )
    .unwrap();
    priority(&app, &a, 1).unwrap();
    app.invoke::<Value>(
        "todo_task_move",
        json!({ "id": a["id"], "listId": work["id"] }),
    )
    .unwrap();
    update(&app, &a, "Secret plan 2", "", "sheet-1");
    update(&app, &a, "Secret plan 2", "details", "sheet-1");
    app.invoke::<Value>(
        "todo_task_wont_do",
        json!({ "id": a["id"], "local": local(TODAY) }),
    )
    .unwrap();
    app.invoke::<Value>("todo_task_reopen", json!({ "id": a["id"] }))
        .unwrap();

    assert_eq!(
        history(&app, &a),
        [
            "Reopened",
            "Won’t do",
            "Title edited",
            "Notes edited",
            "Moved 📥 Inbox → 💼 Work",
            "Priority P4 → P1",
            "Due date Thu 22 Oct → Fri 23 Oct",
            "Time set to 09:30",
            "Due date no date → Thu 22 Oct",
            "Created",
        ]
    );
}

#[test]
fn history_lines_never_hold_the_tasks_own_text() {
    let app = TestApp::new();
    let a = create(&app, "Secret plan");
    update(&app, &a, "Secret plan renamed", "secret notes", "s");
    for line in history(&app, &a) {
        assert!(!line.to_lowercase().contains("secret"), "{line}");
    }
}

#[test]
fn title_and_notes_edits_make_one_line_per_opening_of_the_sheet() {
    let app = TestApp::new();
    let a = create(&app, "a");
    for text in ["h", "he", "hel", "hell", "hello"] {
        update(&app, &a, "a", text, "first-opening");
    }
    assert_eq!(history(&app, &a), ["Notes edited", "Created"]);
    update(&app, &a, "b", "hello", "second-opening");
    update(&app, &a, "bb", "hello", "second-opening");
    assert_eq!(
        history(&app, &a),
        ["Title edited", "Notes edited", "Created"]
    );
}

#[test]
fn one_opening_is_one_line_per_field_even_when_other_changes_happen_in_between() {
    let app = TestApp::new();
    let a = create(&app, "a");
    // Millisecond clock: let each step land in its own millisecond so the order is the real one.
    let pause = || std::thread::sleep(std::time::Duration::from_millis(3));
    pause();
    update(&app, &a, "first", "x", "sheet");
    pause();
    priority(&app, &a, 1).unwrap();
    pause();
    update(&app, &a, "second", "xy", "sheet");
    pause();
    app.invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": a["id"], "date": "2026-10-22", "time": null, "local": local(TODAY) }),
    )
    .unwrap();
    pause();
    update(&app, &a, "third", "xyz", "sheet");
    assert_eq!(
        history(&app, &a),
        [
            "Title edited",
            "Notes edited",
            "Due date no date → Thu 22 Oct",
            "Priority P4 → P1",
            "Created"
        ],
        "one Title edited and one Notes edited, whatever else happened during the opening"
    );
    pause();
    update(&app, &a, "fourth", "xyz", "another");
    assert_eq!(history(&app, &a)[0], "Title edited");
    assert_eq!(history(&app, &a).len(), 6);
}

#[test]
fn a_comment_may_hold_line_breaks_and_up_to_5000_characters() {
    let app = TestApp::new();
    let a = create(&app, "a");
    let c = comment(&app, &a, "one\ntwo");
    assert_eq!(c["body"], "one\ntwo");
    let long = "é".repeat(5000);
    comment(&app, &a, &long);
    assert_eq!(get(&app, &a)["comment_count"], 2);
}

#[test]
fn notes_over_100000_characters_are_refused() {
    let app = TestApp::new();
    let a = create(&app, "a");
    let err = app
        .invoke::<Value>(
            "todo_task_update",
            json!({ "id": a["id"], "title": "a", "notes": "x".repeat(100_001), "sessionId": "s" }),
        )
        .unwrap_err();
    assert_eq!(err["code"], "validation");
    update(&app, &a, "a", &"x".repeat(100_000), "s");
}

#[test]
fn repeats_and_removals_read_naturally() {
    let app = TestApp::new();
    let a = dated(&app, "a", TODAY, Some("08:00"));
    let set = |date: Value, time: Value, repeat: Value| {
        app.invoke::<Value>(
            "todo_task_set_when",
            json!({ "id": a["id"], "date": date, "time": time, "repeat": repeat, "local": local(TODAY) }),
        )
        .unwrap()
    };
    set(
        json!(TODAY),
        json!("08:00"),
        json!({ "freq": "week", "every": 2, "days": 0b100_0010, "mode": "fixed" }),
    );
    set(
        json!(TODAY),
        json!("08:00"),
        json!({ "freq": "day", "every": 3, "days": null, "mode": "after" }),
    );
    set(json!(TODAY), json!("09:00"), Value::Null);
    set(Value::Null, Value::Null, Value::Null);
    assert_eq!(
        history(&app, &a),
        [
            "Due date Wed 21 Oct → no date",
            "Time removed",
            "Time 08:00 → 09:00",
            "Repeat removed",
            "Repeat Every 2 weeks on Mon, Sat → Every 3 days after done",
            "Repeat set: Every 2 weeks on Mon, Sat",
            "Due date no date → Wed 21 Oct",
            "Time set to 08:00",
            "Created",
        ]
    );
}

#[test]
fn completing_a_repeating_task_reads_as_an_occurrence_and_so_does_its_copy() {
    let app = TestApp::new();
    let r = dated(&app, "r", TODAY, None);
    app.invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": r["id"], "date": TODAY, "time": null, "local": local(TODAY),
                "repeat": { "freq": "day", "every": 1, "days": null, "mode": "fixed" } }),
    )
    .unwrap();
    let done = complete(&app, &r).unwrap();
    assert_eq!(
        history(&app, &r)[0],
        "Done (occurrence of 21 Oct)",
        "the next date is not reported as an edit"
    );
    assert_eq!(
        history(&app, &done["occurrence"]),
        ["Done (occurrence of 21 Oct)"]
    );
    app.invoke::<Value>(
        "todo_task_reopen",
        json!({ "id": done["occurrence"]["id"] }),
    )
    .unwrap();
    assert_eq!(history(&app, &r)[0], "Reopened (occurrence of 21 Oct)");
}

#[test]
fn a_skip_reads_as_skipped_and_not_as_an_occurrence_or_a_date_edit() {
    let app = TestApp::new();
    let r = dated(&app, "r", "2026-10-22", None);
    let rule = json!({ "freq": "day", "every": 2, "days": null, "mode": "fixed" });
    app.invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": r["id"], "date": "2026-10-22", "time": null, "local": local(TODAY),
                "repeat": rule }),
    )
    .unwrap();
    app.invoke::<Value>(
        "todo_task_skip",
        json!({ "id": r["id"], "local": local(TODAY) }),
    )
    .unwrap();
    assert_eq!(history(&app, &r)[0], "Skipped (Thu → Sat)");
    // Undo puts the date back with set_when: an ordinary Due date line, not a second skip.
    app.invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": r["id"], "date": "2026-10-22", "time": null, "local": local(TODAY),
                "repeat": rule }),
    )
    .unwrap();
    assert_eq!(history(&app, &r)[0], "Due date Sat 24 Oct → Thu 22 Oct");
    // Completing after a skip is still an occurrence.
    complete(&app, &r).unwrap();
    assert_eq!(history(&app, &r)[0], "Done (occurrence of 22 Oct)");
}

#[test]
fn history_shows_deleted_and_restored_and_names_a_deleted_list() {
    let app = TestApp::new();
    let list: Value = app
        .invoke("todo_list_create", json!({ "name": "Home" }))
        .unwrap();
    let a = create(&app, "a");
    app.invoke::<Value>(
        "todo_task_move",
        json!({ "id": a["id"], "listId": list["id"] }),
    )
    .unwrap();
    app.invoke::<Value>(
        "todo_list_delete",
        json!({ "id": list["id"], "choice": { "kind": "move_to_inbox" } }),
    )
    .unwrap();
    app.invoke::<()>("todo_task_delete", json!({ "id": a["id"] }))
        .unwrap();
    app.invoke::<Value>("todo_task_restore", json!({ "id": a["id"] }))
        .unwrap();
    assert_eq!(
        history(&app, &a),
        [
            "Restored",
            "Deleted",
            "Moved ≡ Home → 📥 Inbox",
            "Moved 📥 Inbox → ≡ Home",
            "Created"
        ]
    );
}

#[test]
fn history_of_an_unknown_task_is_not_found() {
    let app = TestApp::new();
    let err = app
        .invoke::<Value>("todo_task_history", json!({ "id": "nope" }))
        .unwrap_err();
    assert_eq!(err["code"], "not_found");
}

#[test]
fn history_stays_in_the_order_things_happened_when_the_wall_clock_stands_still_or_goes_back() {
    let app = TestApp::new();
    app.clock().freeze(1_800_000_000_000);
    let a = create(&app, "a");
    priority(&app, &a, 1).unwrap();
    app.clock().freeze(1_700_000_000_000);
    priority(&app, &a, 2).unwrap();
    assert_eq!(
        history(&app, &a),
        ["Priority P1 → P2", "Priority P4 → P1", "Created"]
    );
}
