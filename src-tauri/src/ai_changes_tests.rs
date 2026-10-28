//! Command-layer tests for the recent-changes Tools (`journal_changes`, `todo_changes`): a scripted
//! native-tool Provider asks them about changes made through the commands, with the clock frozen so
//! each change is stamped at the instant the test picks (ADR 0008, ADR 0009). No network.

use mnemora_harness::scripted::Reply;
use serde_json::{Value, json};

use crate::ai_tools_tests::{
    ask_in, install, local_now, names, native, payloads, seed_entry, seed_task, sent,
    tool_results_in,
};
use crate::ai_tracer_tests::until_end;
use crate::test_support::TestApp;

/// 2026-10-24 12:00 UTC, which is 13:00 in London: the day before the test's "today".
const YESTERDAY_NOON: i64 = 1_792_843_200_000;

/// The body of a Tool result: the JSON after the observation line, which a refusal may lack.
fn body(result: &str) -> Value {
    let json = result.find('{').map_or("", |at| &result[at..]);
    serde_json::from_str(json).unwrap_or_else(|_| panic!("not a JSON body: {result}"))
}

/// The rows of every day, in the order the Tool gives them.
fn rows(body: &Value) -> Vec<Value> {
    body["days"]
        .as_array()
        .map(|days| {
            days.iter()
                .flat_map(|day| day["rows"].as_array().cloned().unwrap_or_default())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn todo_changes_yesterday_returns_a_done_row_with_its_local_time() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    let task = seed_task(&app, "Book the dentist", None);
    app.invoke::<Value>(
        "todo_task_complete",
        json!({ "id": task, "local": local_now() }),
    )
    .unwrap();

    let results = tool_results_in(
        &app,
        "Europe/London",
        vec![
            Reply::calls(&[("todo_changes", r#"{"from":"2026-10-24","to":"2026-10-24"}"#)]),
            Reply::text(&["Done."]),
        ],
    );

    let body = body(&results[0]);
    assert_eq!(body["days"][0]["date"], "2026-10-24", "{}", results[0]);
    let done = rows(&body)
        .into_iter()
        .find(|row| row["change"] == "Done")
        .unwrap_or_else(|| panic!("no Done row: {}", results[0]));
    assert_eq!(
        done,
        json!({ "id": task, "at": "13:00", "change": "Done", "title": "Book the dentist" })
    );
}

/// A device in New York is on EDT (UTC-4) on the 24th and 25th: 23:50 on the 24th is 03:50 UTC on
/// the 25th, and 00:10 on the 25th is 04:10 UTC, so only the zone tells the two days apart.
const NEW_YORK_2350_ON_24TH: i64 = 1_792_900_200_000;
const NEW_YORK_0010_ON_25TH: i64 = 1_792_901_400_000;

#[test]
fn a_change_belongs_to_the_local_day_it_was_made_on_not_the_utc_day() {
    let app = TestApp::new();
    app.clock().freeze(NEW_YORK_2350_ON_24TH);
    let late = seed_task(&app, "Late night", None);
    app.clock().freeze(NEW_YORK_0010_ON_25TH);
    let early = seed_task(&app, "Just after midnight", None);

    let results = tool_results_in(
        &app,
        "America/New_York",
        vec![
            Reply::calls(&[
                ("todo_changes", r#"{"from":"2026-10-24","to":"2026-10-24"}"#),
                ("todo_changes", r#"{"from":"2026-10-25","to":"2026-10-25"}"#),
            ]),
            Reply::text(&["Done."]),
        ],
    );

    assert_eq!(
        body(&results[0])["days"],
        json!([{ "date": "2026-10-24", "rows": [
            { "id": late, "at": "23:50", "change": "Created", "title": "Late night" }
        ] }]),
        "{}",
        results[0]
    );
    assert_eq!(
        body(&results[1])["days"],
        json!([{ "date": "2026-10-25", "rows": [
            { "id": early, "at": "00:10", "change": "Created", "title": "Just after midnight" }
        ] }]),
        "{}",
        results[1]
    );
}

#[test]
fn a_comment_reads_as_commented_with_a_snippet_of_its_text_on_its_tasks_row() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    let task = seed_task(&app, "Renew passport", None);
    let long = "Photos first, then the form. ".repeat(6);
    app.invoke::<Value>("todo_comment_add", json!({ "taskId": task, "body": long }))
        .unwrap();

    let results = tool_results_in(
        &app,
        "Europe/London",
        vec![
            Reply::calls(&[("todo_changes", r#"{"from":"2026-10-24","to":"2026-10-24"}"#)]),
            Reply::text(&["Done."]),
        ],
    );

    let commented: Vec<Value> = rows(&body(&results[0]))
        .into_iter()
        .filter(|row| {
            row["change"]
                .as_str()
                .is_some_and(|c| c.starts_with("Commented: "))
        })
        .collect();
    assert_eq!(commented.len(), 1, "{}", results[0]);
    assert_eq!(commented[0]["id"], task);
    assert_eq!(commented[0]["at"], "13:00");
    assert_eq!(commented[0]["title"], "Renew passport");
    // "Commented: " is 11 characters; the snippet is cut to 120.
    let change = commented[0]["change"].as_str().unwrap();
    assert_eq!(change.chars().count(), 11 + 120, "{change}");
    assert!(
        change.starts_with("Commented: Photos first, then the form."),
        "{change}"
    );
}

/// 2026-10-20 12:00 UTC: before the range the test asks about.
const BEFORE_THE_RANGE: i64 = 1_792_497_600_000;

#[test]
fn reschedules_priorities_and_moves_read_as_their_history_lines() {
    let app = TestApp::new();
    app.clock().freeze(BEFORE_THE_RANGE);
    let work: Value = app
        .invoke("todo_list_create", json!({ "name": "Work" }))
        .unwrap();
    let task = seed_task(&app, "Pay council tax", Some("2026-10-26"));

    app.clock().freeze(YESTERDAY_NOON);
    app.invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": task, "date": "2026-10-28", "time": null, "local": local_now() }),
    )
    .unwrap();
    app.invoke::<Value>(
        "todo_task_set_priority",
        json!({ "id": task, "priority": 1 }),
    )
    .unwrap();
    app.invoke::<Value>(
        "todo_task_move",
        json!({ "id": task, "listId": work["id"] }),
    )
    .unwrap();

    let results = tool_results_in(
        &app,
        "Europe/London",
        vec![
            Reply::calls(&[("todo_changes", r#"{"from":"2026-10-24","to":"2026-10-24"}"#)]),
            Reply::text(&["Done."]),
        ],
    );

    let changes: Vec<String> = rows(&body(&results[0]))
        .iter()
        .map(|row| row["change"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        changes,
        [
            "Due date Mon 26 Oct → Wed 28 Oct",
            "Priority P4 → P1",
            "Moved 📥 Inbox → ≡ Work",
        ],
        "{}",
        results[0]
    );
}

/// 2026-10-18 12:00 UTC, a Sunday: the day a repeating Task below is due and completed.
const OCTOBER_18_NOON: i64 = 1_792_324_800_000;

/// A Task created and deleted in the range reads as "Created" and "Deleted", each with its title.
#[test]
fn a_created_and_a_deleted_task_read_with_their_titles() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    let task = seed_task(&app, "Call the bank", None);
    app.invoke::<Value>("todo_task_delete", json!({ "id": task }))
        .unwrap();

    let results = tool_results_in(
        &app,
        "Europe/London",
        vec![
            Reply::calls(&[("todo_changes", r#"{"from":"2026-10-24","to":"2026-10-24"}"#)]),
            Reply::text(&["Done."]),
        ],
    );

    let rows = rows(&body(&results[0]));
    let read: Vec<(&str, &str)> = rows
        .iter()
        .map(|row| {
            (
                row["change"].as_str().unwrap(),
                row["title"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        read,
        [("Created", "Call the bank"), ("Deleted", "Call the bank")],
        "{}",
        results[0]
    );
    assert!(rows.iter().all(|row| row["id"] == task), "{}", results[0]);
}

/// Completing a repeating Task on the 18th reads once, on the Task's own row. The occurrence it
/// leaves behind is born done, so it adds no second row.
#[test]
fn a_repeating_completion_reads_as_an_occurrence_once_on_the_series_row() {
    let app = TestApp::new();
    app.clock().freeze(OCTOBER_18_NOON);
    let local = json!({ "today": "2026-10-18", "now": "12:00", "offset_min": 0 });
    let series = seed_task(&app, "Water the plants", None);
    app.invoke::<Value>(
        "todo_task_set_when",
        json!({ "id": series, "date": "2026-10-18", "time": null, "local": local,
                "repeat": { "freq": "day", "every": 1, "days": null, "mode": "fixed" } }),
    )
    .unwrap();
    app.invoke::<Value>(
        "todo_task_complete",
        json!({ "id": series, "local": local }),
    )
    .unwrap();

    let results = tool_results_in(
        &app,
        "Europe/London",
        vec![
            Reply::calls(&[("todo_changes", r#"{"from":"2026-10-18","to":"2026-10-18"}"#)]),
            Reply::text(&["Done."]),
        ],
    );

    let rows = rows(&body(&results[0]));
    let dones: Vec<&Value> = rows
        .iter()
        .filter(|row| row["change"] == "Done (occurrence of 18 Oct)")
        .collect();
    assert_eq!(dones.len(), 1, "{}", results[0]);
    assert_eq!(dones[0]["id"], series);
    assert!(rows.iter().all(|row| row["id"] == series), "{}", results[0]);
}

#[test]
fn counts_cover_each_kind_and_a_kinds_filter_narrows_only_the_rows() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    let first = seed_task(&app, "Book the dentist", None);
    app.invoke::<Value>(
        "todo_task_complete",
        json!({ "id": first, "local": local_now() }),
    )
    .unwrap();
    let second = seed_task(&app, "Renew passport", None);
    app.invoke::<Value>(
        "todo_comment_add",
        json!({ "taskId": second, "body": "Photos first." }),
    )
    .unwrap();

    let results = tool_results_in(
        &app,
        "Europe/London",
        vec![
            Reply::calls(&[
                ("todo_changes", r#"{"from":"2026-10-24","to":"2026-10-24"}"#),
                (
                    "todo_changes",
                    r#"{"from":"2026-10-24","to":"2026-10-24","kinds":"done, commented"}"#,
                ),
                (
                    "todo_changes",
                    r#"{"from":"2026-10-24","to":"2026-10-24","kinds":"sleeping"}"#,
                ),
            ]),
            Reply::text(&["Done."]),
        ],
    );

    let all = body(&results[0]);
    assert_eq!(all["counts"]["created"], 2, "{}", results[0]);
    assert_eq!(all["counts"]["done"], 1, "{}", results[0]);
    assert_eq!(all["counts"]["commented"], 1, "{}", results[0]);
    assert_eq!(all["counts"]["deleted"], 0, "{}", results[0]);
    assert_eq!(rows(&all).len(), 4, "{}", results[0]);

    let filtered = body(&results[1]);
    let changes: Vec<String> = rows(&filtered)
        .iter()
        .map(|row| row["change"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        changes,
        ["Done", "Commented: Photos first."],
        "{}",
        results[1]
    );
    assert_eq!(
        filtered["counts"]["created"], 2,
        "counts cover the whole range"
    );

    assert!(
        results[2].contains("Unknown kind: sleeping. Use: created, done"),
        "{}",
        results[2]
    );
}

#[test]
fn a_long_range_pages_and_a_page_is_cut_to_fit_the_result_cap() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    let task = seed_task(&app, "Long diary", None);
    for i in 0..100 {
        let body = format!("Note {i}: {}", "word ".repeat(40));
        app.invoke::<Value>("todo_comment_add", json!({ "taskId": task, "body": body }))
            .unwrap();
    }

    let first = tool_results_in(
        &app,
        "Europe/London",
        vec![
            Reply::calls(&[
                (
                    "todo_changes",
                    r#"{"from":"2026-10-24","to":"2026-10-24","limit":100}"#,
                ),
                (
                    "todo_changes",
                    r#"{"from":"2026-10-24","to":"2026-10-24","limit":2}"#,
                ),
            ]),
            Reply::text(&["Done."]),
        ],
    );

    assert!(!first[0].contains("too large"), "{}", first[0]);
    assert!(first[0].len() < 16 * 1024, "{}", first[0].len());
    let page = body(&first[0]);
    let shown = rows(&page).len();
    assert!(shown > 0 && shown < 100, "{}", first[0]);
    assert_eq!(page["next_offset"], shown, "{}", first[0]);
    // A day's rows read oldest first: the Task's creation, then its Comments in the order made.
    let page_rows = rows(&page);
    assert_eq!(page_rows[0]["change"], "Created", "{}", first[0]);
    assert!(
        page_rows[1]["change"]
            .as_str()
            .unwrap()
            .starts_with("Commented: Note 0:"),
        "{}",
        first[0]
    );
    let two = body(&first[1]);
    assert_eq!(rows(&two).len(), 2, "{}", first[1]);
    assert_eq!(two["next_offset"], 2, "{}", first[1]);

    let rest = tool_results_in(
        &app,
        "Europe/London",
        vec![
            Reply::calls(&[(
                "todo_changes",
                &format!(
                    r#"{{"from":"2026-10-24","to":"2026-10-24","limit":100,"offset":{shown}}}"#
                ),
            )]),
            Reply::text(&["Done."]),
        ],
    );
    let tail = body(&rest[0]);
    // One Created row and one row per Comment: 101 rows in all.
    assert_eq!(rows(&tail).len(), 101 - shown, "{}", rest[0]);
    assert!(tail["next_offset"].is_null(), "{}", rest[0]);
    let mut seen: Vec<String> = rows(&page)
        .into_iter()
        .chain(rows(&tail))
        .map(|row| row["change"].as_str().unwrap().to_owned())
        .collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 101, "every row appears once across the pages");
}

#[test]
fn an_empty_range_has_no_days_and_zero_counts_and_is_not_an_error() {
    let app = TestApp::new();
    let results = tool_results_in(
        &app,
        "Europe/London",
        vec![
            Reply::calls(&[("todo_changes", r#"{"from":"2026-01-01","to":"2026-01-31"}"#)]),
            Reply::text(&["Nothing changed."]),
        ],
    );

    let empty = body(&results[0]);
    assert_eq!(empty["days"], json!([]), "{}", results[0]);
    assert!(empty["next_offset"].is_null(), "{}", results[0]);
    assert!(
        empty["counts"]
            .as_object()
            .unwrap()
            .values()
            .all(|n| n.as_u64() == Some(0)),
        "{}",
        results[0]
    );
    assert!(empty.get("error").is_none(), "{}", results[0]);
}

#[test]
fn journal_changes_reads_written_edited_once_per_session_and_deleted_with_their_text() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    let kept = seed_entry(&app, "2026-10-24", "Long walk, finally rested.");
    // Two saves in one editing session are one record, so one "Edited" row.
    app.invoke::<Value>(
        "journal_entry_update",
        json!({ "id": kept, "text": "Long walk, finally rested. Then tea.", "sessionId": "edit-1" }),
    )
    .unwrap();
    app.invoke::<Value>(
        "journal_entry_update",
        json!({ "id": kept, "text": "Long walk, finally rested. Then tea and cake.", "sessionId": "edit-1" }),
    )
    .unwrap();
    let gone = seed_entry(&app, "2026-10-24", "A thought I dropped.");
    app.invoke::<Value>("journal_entry_delete", json!({ "id": gone }))
        .unwrap();

    let results = tool_results_in(
        &app,
        "Europe/London",
        vec![
            Reply::calls(&[(
                "journal_changes",
                r#"{"from":"2026-10-24","to":"2026-10-24"}"#,
            )]),
            Reply::text(&["Done."]),
        ],
    );

    assert_eq!(
        rows(&body(&results[0])),
        [
            json!({ "id": kept, "at": "13:00", "change": "Written, 4 words", "title": "Long walk, finally rested." }),
            json!({ "id": kept, "at": "13:00", "change": "Edited", "title": "Long walk, finally rested. Then tea and cake." }),
            json!({ "id": gone, "at": "13:00", "change": "Written, 4 words", "title": "A thought I dropped." }),
            json!({ "id": gone, "at": "13:00", "change": "Deleted", "title": "A thought I dropped." }),
        ],
        "{}",
        results[0]
    );
}

#[test]
fn each_change_tool_is_offered_only_while_its_module_is_on() {
    let app = TestApp::new();
    app.set_pref("core.disabled_modules", json!(["todo"]));
    let provider = native(vec![Reply::text(&["Only the Journal is open."])]);
    install(&app, &provider);
    let events = app.ai_events();
    ask_in(&app, "What changed?", "Europe/London");
    until_end(&events);
    let (tools, _) = sent(&provider, 0);
    let offered = names(&tools);
    assert!(
        offered.contains(&"journal_changes".to_owned()),
        "{offered:?}"
    );
    assert!(!offered.contains(&"todo_changes".to_owned()), "{offered:?}");

    let app = TestApp::new();
    app.set_pref("core.disabled_modules", json!(["journal"]));
    let provider = native(vec![Reply::text(&["Only the Todo is open."])]);
    install(&app, &provider);
    let events = app.ai_events();
    ask_in(&app, "What changed?", "Europe/London");
    until_end(&events);
    let (tools, _) = sent(&provider, 0);
    let offered = names(&tools);
    assert!(offered.contains(&"todo_changes".to_owned()), "{offered:?}");
    assert!(
        !offered.contains(&"journal_changes".to_owned()),
        "{offered:?}"
    );
}

#[test]
fn a_reversed_range_and_an_unknown_zone_are_refused_as_clear_results() {
    let app = TestApp::new();
    let reversed = tool_results_in(
        &app,
        "Europe/London",
        vec![
            Reply::calls(&[
                ("todo_changes", r#"{"from":"2026-10-25","to":"2026-10-24"}"#),
                (
                    "journal_changes",
                    r#"{"from":"2026-10-25","to":"2026-10-24"}"#,
                ),
            ]),
            Reply::text(&["Done."]),
        ],
    );
    for result in &reversed[..2] {
        assert!(
            result.contains("The start date is after the end date."),
            "{result}"
        );
    }

    let unknown = tool_results_in(
        &app,
        "Mars/Olympus",
        vec![
            Reply::calls(&[("todo_changes", r#"{"from":"2026-10-24","to":"2026-10-24"}"#)]),
            Reply::text(&["Done."]),
        ],
    );
    assert!(
        unknown[0].contains("The days could not be read in the device's time zone."),
        "{}",
        unknown[0]
    );
}

#[test]
fn an_id_a_change_tool_returned_is_a_source_and_an_id_it_did_not_return_is_corrected() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    let task = seed_task(&app, "Book the dentist", None);
    let entry = seed_entry(&app, "2026-10-24", "Long walk, finally rested.");
    let made_up = "01990000-0000-7000-8000-000000000000";
    let cases = [
        ("todo_changes", "todo", task.clone()),
        ("journal_changes", "journal", entry.clone()),
    ];
    for (tool, module, id) in cases {
        let provider = native(vec![
            Reply::calls(&[(tool, r#"{"from":"2026-10-24","to":"2026-10-24"}"#)]),
            Reply::text(&["See it [", made_up, "]."]),
            Reply::text(&["See it [", &id, "]."]),
        ]);
        install(&app, &provider);
        let events = app.ai_events();
        let started = ask_in(&app, "What did I do?", "Europe/London");
        until_end(&events);

        let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
        let answer = saved.last().unwrap();
        assert_eq!(answer["text"], format!("See it [{id}]."), "{tool}");
        assert_eq!(answer["sources"][0]["id"], id, "{tool}");
        assert_eq!(answer["sources"][0]["module"], module, "{tool}");
    }
}

#[test]
fn wont_do_reopen_and_skip_read_as_their_history_lines() {
    let app = TestApp::new();
    app.clock().freeze(YESTERDAY_NOON);
    let declined = seed_task(&app, "Learn the violin", None);
    app.invoke::<Value>(
        "todo_task_wont_do",
        json!({ "id": declined, "local": local_now() }),
    )
    .unwrap();
    app.invoke::<Value>("todo_task_reopen", json!({ "id": declined }))
        .unwrap();
    let daily = app
        .invoke::<Value>(
            "todo_task_set_when",
            json!({ "id": seed_task(&app, "Stretch", None), "date": "2026-10-26", "time": null,
                    "local": local_now(),
                    "repeat": { "freq": "day", "every": 1, "days": null, "mode": "fixed" } }),
        )
        .unwrap();
    app.invoke::<Value>(
        "todo_task_skip",
        json!({ "id": daily["id"], "local": local_now() }),
    )
    .unwrap();

    let results = tool_results_in(
        &app,
        "Europe/London",
        vec![
            Reply::calls(&[("todo_changes", r#"{"from":"2026-10-24","to":"2026-10-24"}"#)]),
            Reply::text(&["Done."]),
        ],
    );

    let changes: Vec<String> = rows(&body(&results[0]))
        .iter()
        .map(|row| row["change"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        changes,
        [
            "Created",
            "Won’t do",
            "Reopened",
            "Created",
            "Due date no date → Mon 26 Oct",
            "Repeat set: Every day",
            "Skipped (Mon → Tue)",
        ],
        "{}",
        results[0]
    );
}
