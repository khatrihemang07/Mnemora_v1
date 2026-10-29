//! Command-layer tests on the catch-up day of the recent-changes fixture: 2026-10-06 in New York
//! (fixture day -19), with 134 changes and 5 Journal rows, more than one default batch holds
//! (2026-10-29 follow-up, decision 8; ADR 0009). The Tools page through `next_offset`, and a Run that
//! stops after the first batch carries the app's note. A scripted native-tool Provider asks the
//! Tools through the commands. No network.

use std::collections::HashSet;

use mnemora_harness::scripted::Reply;
use serde_json::{Value, json};

use crate::ai_fixture::{self, Loaded};
use crate::ai_tools_tests::{ask_in, install, native, payloads, tool_results_in};
use crate::ai_tracer_tests::until_end;
use crate::test_support::TestApp;

/// The device's zone, as in `ai_fixture_tests`.
const ZONE: &str = "America/New_York";
/// The fixed now: 2026-10-25 12:00 in New York, as in `ai_fixture_tests`.
const NOW: i64 = 1_792_944_000_000;
/// The catch-up day: fixture day -19, Tuesday 6 October.
const DAY: &str = r#""from":"2026-10-06","to":"2026-10-06""#;
/// The app's note for the first batch of this day's Tasks, read by `todo_changes` at its default.
const NOTE: &str = "Based on the first 100 of 134 changes (Tasks); the rest was not read.";

/// The fixture loaded into a fresh app, with the clock at `NOW`.
fn loaded() -> (TestApp, Loaded) {
    let app = TestApp::new();
    let ids = ai_fixture::load(&app, NOW, ZONE).unwrap();
    assert_eq!(ids.len(), 112, "the fixture did not load in full");
    (app, ids)
}

/// The body of a Tool result: the JSON after the observation line.
fn body(result: &str) -> Value {
    let json = result.find('{').map_or("", |at| &result[at..]);
    serde_json::from_str(json).unwrap_or_else(|_| panic!("not a JSON body: {result}"))
}

/// One row of a changes Tool, as the Tool gives it.
fn row(id: &str, at: &str, change: &str, title: &str) -> Value {
    json!({ "id": id, "at": at, "change": change, "title": title })
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

/// Runs a question on a scripted native-tool Provider and returns the Run's saved payloads.
fn saved_run(app: &TestApp, replies: Vec<Reply>) -> Vec<Value> {
    let provider = native(replies);
    install(app, &provider);
    let events = app.ai_events();
    let started = ask_in(app, "What did I do on the 6th?", ZONE);
    until_end(&events);
    payloads(app, started["conversation_id"].as_str().unwrap())
}

/// How many saved entries carry a note, which must be one per Run at most.
fn notes_in(saved: &[Value]) -> usize {
    saved.iter().filter(|p| !p["budget_note"].is_null()).count()
}

#[test]
fn the_default_batch_of_the_catch_up_day_holds_100_rows_and_says_it_is_incomplete() {
    let (app, ids) = loaded();
    let id = |key: &str| ids.id(key).to_owned();
    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[("todo_changes", &format!("{{{DAY}}}"))]),
            Reply::text(&["Done."]),
        ],
    );

    let first = body(&results[0]);
    let rows = rows_of(&first);
    assert_eq!(rows.len(), 100, "{}", results[0]);
    assert_eq!(first["shown"], 100, "{}", results[0]);
    assert_eq!(first["total"], 134, "{}", results[0]);
    assert_eq!(first["next_offset"], 100, "{}", results[0]);
    assert_eq!(
        first["note"],
        "100 of 134 changes shown. The list is incomplete: call again with offset 100 for the rest.",
        "{}",
        results[0]
    );
    // The batch is well under the 16 KiB cap, so the cap cuts nothing here.
    assert!(results[0].len() < 16 * 1024, "{}", results[0].len());
    assert_eq!(
        first["counts"],
        json!({ "created": 61, "done": 53, "wont": 1, "skipped": 0, "reopened": 0, "moved": 1, "due": 0, "priority": 0, "edited": 0, "commented": 16, "deleted": 1, "restored": 0, "repeat": 1 }),
        "{}",
        results[0]
    );
    assert_eq!(
        rows[0],
        row(&id("cu01"), "07:00", "Created", "Clean the oven"),
        "{}",
        results[0]
    );
    assert_eq!(
        rows[99],
        row(&id("cu42"), "15:30", "Done", "Scan the old photos"),
        "{}",
        results[0]
    );
}

#[test]
fn following_next_offset_to_the_end_yields_every_row_once_and_the_last_batch_has_no_note() {
    let (app, ids) = loaded();
    let id = |key: &str| ids.id(key).to_owned();
    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[
                ("todo_changes", &format!("{{{DAY}}}")),
                ("todo_changes", &format!("{{{DAY},\"offset\":100}}")),
            ]),
            Reply::text(&["Done."]),
        ],
    );

    let first = body(&results[0]);
    assert_eq!(first["next_offset"], 100, "{}", results[0]);
    let last = body(&results[1]);
    assert_eq!(rows_of(&last).len(), 34, "{}", results[1]);
    assert_eq!(last["shown"], 34, "{}", results[1]);
    assert_eq!(last["total"], 134, "{}", results[1]);
    assert!(last["next_offset"].is_null(), "{}", results[1]);
    assert!(last.get("note").is_none(), "{}", results[1]);

    let mut rows = rows_of(&first);
    rows.extend(rows_of(&last));
    assert_eq!(rows.len(), 134, "the two batches hold the total");
    let distinct: HashSet<String> = rows.iter().map(Value::to_string).collect();
    assert_eq!(distinct.len(), 134, "a row came in two batches");
    assert_eq!(
        rows[100],
        row(&id("cu43"), "15:35", "Created", "Wipe down the bins"),
        "{}",
        results[1]
    );
    assert_eq!(
        rows[133],
        row(
            &id("cubed"),
            "18:30",
            "Done (occurrence of 6 Oct)",
            "Change the bedding"
        ),
        "{}",
        results[1]
    );
}

#[test]
fn journal_changes_for_the_catch_up_day_are_complete_and_have_no_note() {
    let (app, ids) = loaded();
    let id = |key: &str| ids.id(key).to_owned();
    let results = tool_results_in(
        &app,
        ZONE,
        vec![
            Reply::calls(&[("journal_changes", &format!("{{{DAY}}}"))]),
            Reply::text(&["Done."]),
        ],
    );

    // A row's title is the Entry's first 120 characters.
    let journal = body(&results[0]);
    assert_eq!(
        rows_of(&journal),
        [
            row(&id("cue1"), "09:25", "Written, 25 words", "Slept badly again, so I made tea and sat by the window until it got light. The street was empty apart from the bin lorry"),
            row(&id("cue2"), "12:30", "Written, 32 words", "Lunch in the park with Sam. We talked about the new job and whether to take it. I think I already know what I want, I ju"),
            row(&id("cue3"), "15:05", "Written, 20 words", "Long queue at the post office, then a walk home through the market. Bought fresh bread and too many lemons."),
            row(&id("cue2"), "16:00", "Edited", "Lunch in the park with Sam. We talked about the new job and whether to take it. I think I already know what I want, and "),
            row(&id("cue4"), "17:35", "Written, 20 words", "Quiet evening. Finished the book on the sofa and went to bed before eleven, which felt like a small win."),
        ]
        .to_vec(),
        "{}",
        results[0]
    );
    assert_eq!(
        journal["counts"],
        json!({ "written": 4, "edited": 1, "deleted": 0 }),
        "{}",
        results[0]
    );
    assert!(journal["next_offset"].is_null(), "{}", results[0]);
    assert!(journal.get("note").is_none(), "{}", results[0]);
}

#[test]
fn a_run_that_stops_after_the_first_batch_says_the_rest_was_not_read() {
    let (app, _ids) = loaded();
    let saved = saved_run(
        &app,
        vec![
            Reply::calls(&[("todo_changes", &format!("{{{DAY}}}"))]),
            Reply::text(&["Done."]),
        ],
    );

    assert_eq!(saved.last().unwrap()["budget_note"], NOTE);
    assert_eq!(notes_in(&saved), 1, "the note is kept once");
}

#[test]
fn a_run_that_follows_the_batches_to_the_end_has_no_note() {
    let (app, _ids) = loaded();
    let saved = saved_run(
        &app,
        vec![
            Reply::calls(&[("todo_changes", &format!("{{{DAY}}}"))]),
            Reply::calls(&[("todo_changes", &format!("{{{DAY},\"offset\":100}}"))]),
            Reply::text(&["Done."]),
        ],
    );

    assert!(saved.last().unwrap()["budget_note"].is_null());
    assert_eq!(notes_in(&saved), 0);
}

#[test]
fn leaving_out_the_catch_up_day_writes_every_other_day_and_none_of_its_keys() {
    let app = TestApp::new();
    let loaded = ai_fixture::load_except(&app, NOW, ZONE, Some(ai_fixture::CATCHUP_DAY)).unwrap();
    assert_eq!(loaded.left_out(), 139, "the catch-up day has 139 events");
    assert_eq!(loaded.events(), 221 - 139, "the fixture has 221 events");
    assert_eq!(
        loaded.len(),
        112 - 65,
        "the catch-up day creates 65 keyed items"
    );
    assert!(!loaded.contains("cu01"), "a catch-up key was written");
    assert!(loaded.contains("hartley"), "another day's key was left out");
    let whole = ai_fixture::load(&TestApp::new(), NOW, ZONE).unwrap();
    assert_eq!(whole.left_out(), 0);
    assert!(whole.contains("cu01"));
}
