//! Command-layer tests for Task search (ADR 0009): the shared fixture cases the browser mock also
//! runs, plus what only the real database can show (the index staying in step with edits).

use serde_json::{Value, json};

use crate::test_support::TestApp;

const RULES: &str = include_str!("../../src/lib/ipc/fixtures/todo-search-rules.json");

fn local() -> Value {
    json!({ "today": "2026-10-21", "now": "14:05", "offset_min": 0 })
}

fn create(app: &TestApp, title: &str) -> Value {
    app.invoke("todo_task_create", json!({ "title": title }))
        .unwrap()
}

fn comment(app: &TestApp, task: &Value, body: &str) -> Value {
    app.invoke(
        "todo_comment_add",
        json!({ "taskId": task["id"], "body": body }),
    )
    .unwrap()
}

fn set_notes(app: &TestApp, task: &Value, title: &str, notes: &str) {
    app.invoke::<Value>(
        "todo_task_update",
        json!({ "id": task["id"], "title": title, "notes": notes, "sessionId": "s" }),
    )
    .unwrap();
}

fn search(app: &TestApp, query: &str) -> Value {
    app.invoke("todo_search", json!({ "query": query }))
        .unwrap()
}

fn titles(hits: &Value) -> Vec<String> {
    hits.as_array()
        .unwrap()
        .iter()
        .map(|h| h["task"]["title"].as_str().unwrap().to_owned())
        .collect()
}

fn spans(spans: &Value) -> (String, Vec<String>) {
    let all = spans.as_array().unwrap();
    let text = all.iter().map(|s| s["text"].as_str().unwrap()).collect();
    let marked = all
        .iter()
        .filter(|s| s["matched"] == true)
        .map(|s| s["text"].as_str().unwrap().to_owned())
        .collect();
    (text, marked)
}

fn exact(app: &TestApp, query: &str) -> Vec<String> {
    titles(&search(app, query)["exact"])
}

fn build(app: &TestApp, tasks: &[Value]) {
    for t in tasks {
        let task = create(app, t["title"].as_str().unwrap());
        if let Some(notes) = t["notes"].as_str() {
            set_notes(app, &task, t["title"].as_str().unwrap(), notes);
        }
        for body in t["comments"].as_array().into_iter().flatten() {
            comment(app, &task, body.as_str().unwrap());
        }
        for body in t["deleted_comments"].as_array().into_iter().flatten() {
            let c = comment(app, &task, body.as_str().unwrap());
            app.invoke::<()>("todo_comment_delete", json!({ "id": c["id"] }))
                .unwrap();
        }
        match t["state"].as_str().unwrap_or("open") {
            "done" => {
                app.invoke::<Value>(
                    "todo_task_complete",
                    json!({ "id": task["id"], "local": local() }),
                )
                .unwrap();
            }
            "wont" => {
                app.invoke::<Value>("todo_task_wont_do", json!({ "id": task["id"] }))
                    .unwrap();
            }
            "deleted" => app
                .invoke::<()>("todo_task_delete", json!({ "id": task["id"] }))
                .unwrap(),
            _ => {}
        }
    }
}

#[test]
fn the_shared_search_fixtures_hold() {
    let rules: Value = serde_json::from_str(RULES).unwrap();
    for case in rules["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let app = TestApp::new();
        build(&app, case["tasks"].as_array().unwrap());
        for q in case["queries"].as_array().unwrap() {
            let found = search(&app, q["q"].as_str().unwrap());
            assert_eq!(
                Value::from(titles(&found["exact"])),
                q["exact"],
                "{name}: exact for {}",
                q["q"]
            );
            assert_eq!(
                Value::from(titles(&found["similar"])),
                q["similar"],
                "{name}: similar for {}",
                q["q"]
            );
        }
        for (title, m) in case["marked"].as_object().into_iter().flatten() {
            let found = search(&app, m["q"].as_str().unwrap());
            let hit = found["exact"]
                .as_array()
                .unwrap()
                .iter()
                .find(|h| h["task"]["title"] == *title)
                .unwrap();
            assert_eq!(Value::from(spans(&hit["title"]).1), m["words"], "{name}");
        }
        for (title, s) in case["snippets"].as_object().into_iter().flatten() {
            let found = search(&app, s["q"].as_str().unwrap());
            let hit = found["exact"]
                .as_array()
                .unwrap()
                .iter()
                .find(|h| h["task"]["title"] == *title)
                .unwrap();
            let (text, marked) = spans(&hit["snippet"]);
            assert_eq!(hit["snippet_source"], s["source"], "{name}: source");
            assert_eq!(Value::from(marked), s["marked"], "{name}: marked");
            assert_eq!(Value::from(text), s["text"], "{name}: text");
        }
    }
}

#[test]
fn the_index_follows_edits_to_titles_notes_and_comments() {
    let app = TestApp::new();
    let task = create(&app, "Alpha task");
    assert_eq!(exact(&app, "alpha"), ["Alpha task"]);

    set_notes(&app, &task, "Bravo task", "mentions kiwi");
    assert!(exact(&app, "alpha").is_empty());
    assert_eq!(exact(&app, "bravo"), ["Bravo task"]);
    assert_eq!(exact(&app, "kiwi"), ["Bravo task"]);

    set_notes(&app, &task, "Bravo task", "");
    assert!(exact(&app, "kiwi").is_empty());

    let c = comment(&app, &task, "old words");
    assert_eq!(exact(&app, "old"), ["Bravo task"]);
    app.invoke::<Value>(
        "todo_comment_edit",
        json!({ "id": c["id"], "body": "new words" }),
    )
    .unwrap();
    assert!(exact(&app, "old").is_empty());
    assert_eq!(exact(&app, "new"), ["Bravo task"]);

    // Delete hides, Undo brings it back.
    app.invoke::<()>("todo_comment_delete", json!({ "id": c["id"] }))
        .unwrap();
    assert!(exact(&app, "new").is_empty());
    app.invoke::<Value>("todo_comment_restore", json!({ "id": c["id"] }))
        .unwrap();
    assert_eq!(exact(&app, "new"), ["Bravo task"]);

    app.invoke::<()>("todo_task_delete", json!({ "id": task["id"] }))
        .unwrap();
    assert!(exact(&app, "bravo").is_empty());
    assert!(exact(&app, "new").is_empty());
    app.invoke::<Value>("todo_task_restore", json!({ "id": task["id"] }))
        .unwrap();
    assert_eq!(exact(&app, "bravo"), ["Bravo task"]);
}

#[test]
fn open_tasks_are_listed_before_done_ones() {
    let app = TestApp::new();
    let task = create(&app, "Ship report");
    create(&app, "Report bugs");
    app.invoke::<Value>(
        "todo_task_complete",
        json!({ "id": task["id"], "local": local() }),
    )
    .unwrap();
    let found = search(&app, "report");
    let states: Vec<_> = found["exact"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["task"]["state"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(states, ["open", "done"]);
}

#[test]
fn search_never_logs_or_records_and_hostile_input_is_not_an_error() {
    let app = TestApp::new();
    create(&app, "Anything");
    let before: i64 = app.with_db(|c| {
        c.query_row("SELECT COUNT(*) FROM change_log", [], |r| r.get(0))
            .unwrap()
    });
    for q in ["\"", "*", "NEAR(", "a:b", "x OR", "(", "\"\"\"", "-"] {
        let found = search(&app, q);
        assert!(found["exact"].is_array(), "{q}");
    }
    let after: i64 = app.with_db(|c| {
        c.query_row("SELECT COUNT(*) FROM change_log", [], |r| r.get(0))
            .unwrap()
    });
    assert_eq!(before, after);
}

#[test]
fn long_notes_give_a_windowed_snippet_around_the_match() {
    let app = TestApp::new();
    let task = create(&app, "Long");
    let notes = format!("{} needle {}", "word ".repeat(60), "tail ".repeat(60));
    set_notes(&app, &task, "Long", &notes);
    let found = search(&app, "needle");
    let (text, marked) = spans(&found["exact"][0]["snippet"]);
    assert_eq!(marked, ["needle"]);
    assert!(text.starts_with('…') && text.ends_with('…'));
    assert!(text.chars().count() < 200);
}
