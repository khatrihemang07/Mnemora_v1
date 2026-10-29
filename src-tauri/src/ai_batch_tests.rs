//! Command-layer tests for batches: a changes Tool's result says how many rows it shows and how many
//! the query matches, and when it is incomplete, a note that says where the next batch starts
//! (2026-10-29 follow-up, decisions 1 and 2; ADR 0009). A scripted native-tool Provider asks the
//! Tools through the commands, with the clock frozen so every change lands on one local day. No
//! network.

use mnemora_core::recent::CHANGES_DEFAULT;
use mnemora_harness::scripted::Reply;
use serde_json::{Value, json};

use crate::ai_tools_tests::{local_now, seed_task, tool_results_in};
use crate::test_support::TestApp;

/// 2026-10-24 12:00 UTC, which is 13:00 in London: the day every test asks about.
const YESTERDAY_NOON: i64 = 1_792_843_200_000;
const ZONE: &str = "Europe/London";
const DAY: &str = r#""from":"2026-10-24","to":"2026-10-24""#;

/// The body of a Tool result: the JSON after the observation line.
fn body(result: &str) -> Value {
    let json = result.find('{').map_or("", |at| &result[at..]);
    serde_json::from_str(json).unwrap_or_else(|_| panic!("not a JSON body: {result}"))
}

/// Every row of a changes body, in the order the Tool gives them.
fn rows_of(body: &Value) -> Vec<Value> {
    body["days"]
        .as_array()
        .map(|days| {
            days.iter()
                .flat_map(|day| day["rows"].as_array().cloned().unwrap_or_default())
                .collect()
        })
        .unwrap_or_default()
}

/// The ids of a changes body's rows, in the order the Tool gives them.
fn ids_of(body: &Value) -> Vec<String> {
    rows_of(body)
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_owned())
        .collect()
}

/// Creates one Task per title on the frozen day, in order, and returns their ids.
fn seed_tasks(app: &TestApp, titles: impl Iterator<Item = String>) -> Vec<String> {
    titles.map(|title| seed_task(app, &title, None)).collect()
}

#[test]
fn the_default_batch_holds_100_rows_and_says_the_list_is_incomplete() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    seed_tasks(&app, (0..130).map(|i| format!("Task {i:03}")));

    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[("todo_changes", &format!("{{{DAY}}}"))]),
            Reply::text(&["Done."]),
        ],
    );

    let first = body(&results[0]);
    assert_eq!(rows_of(&first).len(), 100, "{}", results[0]);
    assert_eq!(first["shown"], 100, "{}", results[0]);
    assert_eq!(first["total"], 130, "{}", results[0]);
    assert_eq!(first["next_offset"], 100, "{}", results[0]);
    assert_eq!(
        first["note"],
        "100 of 130 changes shown. The list is incomplete: call again with offset 100 for the rest.",
        "{}",
        results[0]
    );
}

#[test]
fn the_batch_from_offset_100_holds_the_rest_and_no_row_is_lost_or_repeated() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    let seeded = seed_tasks(&app, (0..130).map(|i| format!("Task {i:03}")));

    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[
                ("todo_changes", &format!("{{{DAY}}}")),
                (
                    "todo_changes",
                    &format!("{{{DAY},\"offset\":{CHANGES_DEFAULT}}}"),
                ),
            ]),
            Reply::text(&["Done."]),
        ],
    );

    let second = body(&results[1]);
    assert_eq!(rows_of(&second).len(), 30, "{}", results[1]);
    assert_eq!(second["shown"], 30, "{}", results[1]);
    assert_eq!(second["total"], 130, "{}", results[1]);
    assert!(second["next_offset"].is_null(), "{}", results[1]);
    assert!(second.get("note").is_none(), "{}", results[1]);

    let mut ids = ids_of(&body(&results[0]));
    ids.extend(ids_of(&second));
    assert_eq!(
        ids, seeded,
        "the two batches hold every Task once, in order"
    );
}

#[test]
fn a_complete_result_has_shown_equal_total_and_no_note() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    seed_tasks(&app, (0..3).map(|i| format!("Task {i}")));

    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[("todo_changes", &format!("{{{DAY}}}"))]),
            Reply::text(&["Done."]),
        ],
    );

    let whole = body(&results[0]);
    assert_eq!(rows_of(&whole).len(), 3, "{}", results[0]);
    assert_eq!(whole["shown"], 3, "{}", results[0]);
    assert_eq!(whole["total"], 3, "{}", results[0]);
    assert!(whole["next_offset"].is_null(), "{}", results[0]);
    assert!(whole.get("note").is_none(), "{}", results[0]);
}

#[test]
fn a_batch_cut_to_the_result_cap_counts_only_the_rows_it_shows() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    seed_tasks(
        &app,
        (0..130).map(|i| format!("Task {i:03} {}", "word ".repeat(80))),
    );

    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[("todo_changes", &format!("{{{DAY}}}"))]),
            Reply::text(&["Done."]),
        ],
    );

    assert!(results[0].len() < 16 * 1024, "{}", results[0].len());
    let cut = body(&results[0]);
    let shown = rows_of(&cut).len();
    assert!(
        shown > 0 && shown < 100,
        "the cap cut nothing: {}",
        results[0]
    );
    assert_eq!(cut["shown"], shown, "{}", results[0]);
    assert_eq!(cut["total"], 130, "{}", results[0]);
    assert_eq!(cut["next_offset"], shown, "{}", results[0]);
    assert_eq!(
        cut["note"],
        format!(
            "{shown} of 130 changes shown. The list is incomplete: call again with offset {shown} for the rest."
        ),
        "{}",
        results[0]
    );
}

#[test]
fn a_kinds_filter_counts_only_the_matching_rows_as_total() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    let seeded = seed_tasks(&app, (0..5).map(|i| format!("Task {i}")));
    for id in &seeded[..2] {
        app.invoke::<Value>(
            "todo_task_complete",
            json!({ "id": id, "local": local_now() }),
        )
        .unwrap();
    }

    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[
                ("todo_changes", &format!("{{{DAY},\"kinds\":\"done\"}}")),
                (
                    "todo_changes",
                    &format!("{{{DAY},\"kinds\":\"created\",\"limit\":3}}"),
                ),
            ]),
            Reply::text(&["Done."]),
        ],
    );

    let done = body(&results[0]);
    assert_eq!(rows_of(&done).len(), 2, "{}", results[0]);
    assert_eq!(done["shown"], 2, "{}", results[0]);
    assert_eq!(done["total"], 2, "{}", results[0]);
    assert_eq!(done["counts"]["created"], 5, "{}", results[0]);
    assert_eq!(done["counts"]["done"], 2, "{}", results[0]);
    assert!(done.get("note").is_none(), "{}", results[0]);

    let created = body(&results[1]);
    assert_eq!(created["shown"], 3, "{}", results[1]);
    assert_eq!(created["total"], 5, "{}", results[1]);
    assert_eq!(created["next_offset"], 3, "{}", results[1]);
    assert_eq!(
        created["note"],
        "3 of 5 changes shown. The list is incomplete: call again with offset 3 for the rest.",
        "{}",
        results[1]
    );
}

#[test]
fn a_later_batch_that_is_still_incomplete_counts_from_the_start_of_the_list() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    seed_tasks(&app, (0..130).map(|i| format!("Task {i:03}")));

    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[(
                "todo_changes",
                &format!("{{{DAY},\"limit\":30,\"offset\":30}}"),
            )]),
            Reply::text(&["Done."]),
        ],
    );

    let middle = body(&results[0]);
    assert_eq!(rows_of(&middle).len(), 30, "{}", results[0]);
    assert_eq!(middle["shown"], 30, "{}", results[0]);
    assert_eq!(middle["total"], 130, "{}", results[0]);
    assert_eq!(middle["next_offset"], 60, "{}", results[0]);
    assert_eq!(
        middle["note"],
        "60 of 130 changes shown. The list is incomplete: call again with offset 60 for the rest.",
        "{}",
        results[0]
    );
}

#[test]
fn the_same_call_with_a_new_offset_is_run_and_not_refused_as_a_repeat() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    seed_tasks(&app, (0..130).map(|i| format!("Task {i:03}")));

    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[("todo_changes", &format!("{{{DAY}}}"))]),
            Reply::calls(&[(
                "todo_changes",
                &format!("{{{DAY},\"offset\":{CHANGES_DEFAULT}}}"),
            )]),
            Reply::text(&["Done."]),
        ],
    );

    assert!(!results[1].contains("This repeats call"), "{}", results[1]);
    let second = body(&results[1]);
    assert_eq!(second["shown"], 30, "{}", results[1]);
    assert_eq!(second["total"], 130, "{}", results[1]);
}
