//! Tests for the Comment hints on the `todo_list` and `todo_search` rows: a Task with live Comments
//! carries `comments`, `last_comment_at` and `last_comment_at_local`; a Task without them carries
//! none of the three; deleted Comments are not counted; and a page of such rows stays under the
//! result cap. Comments take the real clock, so the newest instant is read back from the database
//! and the local reading is derived from it with the device's zone.

use std::time::Duration;

use mnemora_core::local_time;
use mnemora_core::tool::RESULT_CAP;
use mnemora_harness::scripted::Reply;
use serde_json::{Value, json};

use crate::ai_tools_tests::{seed_task, tool_results_in};
use crate::test_support::TestApp;

/// The device's zone: London in October is UTC+1.
const ZONE: &str = "Europe/London";

/// The body of a Tool result: the JSON after the observation line.
fn body(result: &str) -> Value {
    let json = result.find('{').map_or("", |at| &result[at..]);
    serde_json::from_str(json).unwrap_or_else(|_| panic!("not a JSON body: {result}"))
}

/// The row of `task` in a list or search body.
fn row_of<'a>(body: &'a Value, task: &str) -> &'a Value {
    body["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == task)
        .unwrap_or_else(|| panic!("no row for {task} in {body}"))
}

/// Adds a Comment to `task`, then waits a moment so the next one has a later instant.
fn comment(app: &TestApp, task: &str, text: &str) {
    app.invoke::<Value>("todo_comment_add", json!({ "taskId": task, "body": text }))
        .unwrap();
    std::thread::sleep(Duration::from_millis(3));
}

/// The live Comments of `task` as (id, `created_at`), oldest first, read from the database.
fn live_comments(app: &TestApp, task: &str) -> Vec<(String, i64)> {
    app.with_db(|conn| {
        let mut statement = conn
            .prepare(
                "SELECT id, created_at FROM todo_comments WHERE task_id = ?1 AND deleted_at IS NULL ORDER BY created_at, id",
            )
            .unwrap();
        statement
            .query_map([task], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    })
}

/// Runs `todo_search` for `query` and `todo_list` for open Tasks, and returns both bodies.
fn search_and_list(app: &TestApp, query: &str) -> (Value, Value) {
    let results = tool_results_in(
        app,
        ZONE,
        vec![
            Reply::calls(&[
                ("todo_search", &format!(r#"{{"query":"{query}"}}"#)),
                ("todo_list", r#"{"state":"open"}"#),
            ]),
            Reply::text(&["Done."]),
        ],
    );
    (body(&results[0]), body(&results[1]))
}

#[test]
fn a_task_with_two_comments_shows_their_count_and_newest_time_in_search_and_list() {
    let app = TestApp::new();
    let task = seed_task(&app, "Renew passport", Some("2026-10-30"));
    comment(&app, &task, "Photos first, then the form.");
    comment(
        &app,
        &task,
        "Sent the form off this morning. Still waiting on the photos.",
    );
    let comments = live_comments(&app, &task);
    assert_eq!(comments.len(), 2);
    let newest = comments[1].1;
    let newest_local = local_time::minute(newest, ZONE).unwrap();

    let (search, list) = search_and_list(&app, "passport");
    for found in [row_of(&search, &task), row_of(&list, &task)] {
        assert_eq!(found["comments"], 2, "{found}");
        assert_eq!(found["last_comment_at"], newest, "{found}");
        assert_eq!(
            found["last_comment_at_local"],
            newest_local.as_str(),
            "{found}"
        );
    }
}

#[test]
fn a_task_without_comments_carries_none_of_the_comment_hints() {
    let app = TestApp::new();
    let task = seed_task(&app, "Renew passport", Some("2026-10-30"));

    let (search, list) = search_and_list(&app, "passport");
    for found in [row_of(&search, &task), row_of(&list, &task)] {
        assert!(found.get("comments").is_none(), "{found}");
        assert!(found.get("last_comment_at").is_none(), "{found}");
        assert!(found.get("last_comment_at_local").is_none(), "{found}");
    }
}

#[test]
fn deleted_comments_are_not_counted_and_the_hints_go_when_none_is_left() {
    let app = TestApp::new();
    let task = seed_task(&app, "Renew passport", Some("2026-10-30"));
    comment(&app, &task, "Photos first, then the form.");
    comment(&app, &task, "Sent the form off this morning.");
    let before = live_comments(&app, &task);
    let (older, newest) = (before[0].clone(), before[1].clone());
    app.invoke::<Value>("todo_comment_delete", json!({ "id": newest.0 }))
        .unwrap();

    let (search, _) = search_and_list(&app, "passport");
    let found = row_of(&search, &task);
    assert_eq!(found["comments"], 1, "{found}");
    assert_eq!(found["last_comment_at"], older.1, "{found}");

    app.invoke::<Value>("todo_comment_delete", json!({ "id": older.0 }))
        .unwrap();
    let (_, list) = search_and_list(&app, "passport");
    let found = row_of(&list, &task);
    assert!(found.get("comments").is_none(), "{found}");
    assert!(found.get("last_comment_at").is_none(), "{found}");
}

#[test]
fn a_full_page_of_commented_rows_stays_under_the_result_cap() {
    let app = TestApp::new();
    let mut tasks = Vec::new();
    for n in 0..30 {
        let task = seed_task(
            &app,
            &format!("Long running task number {n} with a title"),
            None,
        );
        comment(&app, &task, "First note on this task.");
        comment(&app, &task, "Second note on this task.");
        tasks.push(task);
    }
    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[("todo_list", r#"{"state":"open"}"#)]),
            Reply::text(&["Done."]),
        ],
    );
    assert!(
        results[0].len() < RESULT_CAP,
        "the page is {} bytes",
        results[0].len()
    );
    assert!(!results[0].contains("too large"), "{}", results[0]);
    let page = body(&results[0]);
    let rows = page["tasks"].as_array().unwrap();
    assert_eq!(rows.len(), 30, "{}", results[0]);
    assert!(
        rows.iter().all(|row| row["comments"] == 2),
        "{}",
        results[0]
    );
}
