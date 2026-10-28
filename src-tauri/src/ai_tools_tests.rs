//! Command-layer tests for the AI Tools (#87): a scripted native-tool Provider calls the six read-only
//! Tools against real Journal and Todo data through `ai_run_start`, and the saved entries, the typed
//! run events and the Sources are read back. No network: the scripted adapter is the Model (ADR 0009).

use std::sync::Arc;

use mnemora_harness::scripted::{Reply, ScriptedProvider};
use mnemora_harness::{Provider, ToolDef};
use serde_json::{Value, json};

use crate::ai_tracer_tests::{ended_outcome, label, until_end};
use crate::test_support::TestApp;
use mnemora_ai::RunOutcome;

pub(crate) const TODAY: &str = "2026-10-25";

pub(crate) fn local_now() -> Value {
    json!({ "today": TODAY, "now": "12:00", "offset_min": 0 })
}

fn local() -> Value {
    json!({
        "today": TODAY,
        "weekday": "Sunday",
        "time_zone": "Europe/London",
        "week_start": "mon",
    })
}

/// A Provider that makes native Tool calls, playing `replies` one Step at a time.
pub(crate) fn native(replies: Vec<Reply>) -> Arc<ScriptedProvider> {
    Arc::new(ScriptedProvider::new("Scripted", "scripted-model", replies).with_native_tools())
}

pub(crate) fn install(app: &TestApp, provider: &Arc<ScriptedProvider>) {
    let provider: Arc<dyn Provider> = provider.clone();
    app.set_ai_provider(provider);
    app.invoke::<Value>(
        "ai_provider_create",
        json!({ "request": { "kind": "wrapper", "name": "Scripted", "address": "http://127.0.0.1:9/v1" } }),
    )
    .unwrap();
}

pub(crate) fn provider_id(app: &TestApp) -> String {
    let list: Value = app.invoke("ai_provider_list", json!({})).unwrap();
    list[0]["id"].as_str().unwrap().to_owned()
}

fn ask(app: &TestApp, text: &str) -> Value {
    app.invoke(
        "ai_run_start",
        json!({ "request": {
            "conversation_id": null,
            "text": text,
            "provider_id": provider_id(app),
            "model": "scripted-model",
            "local": local(),
        } }),
    )
    .unwrap()
}

pub(crate) fn payloads(app: &TestApp, conversation: &str) -> Vec<Value> {
    let view: Value = app
        .invoke("ai_conversation_get", json!({ "id": conversation }))
        .unwrap();
    view["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["payload"].clone())
        .collect()
}

/// The Tools and system prompt a Step was sent with.
pub(crate) fn sent(provider: &ScriptedProvider, step: usize) -> (Vec<ToolDef>, String) {
    let request = &provider.requests()[step];
    (request.tools.clone(), request.system.clone())
}

pub(crate) fn names(tools: &[ToolDef]) -> Vec<String> {
    tools.iter().map(|t| t.name.clone()).collect()
}

pub(crate) fn seed_entry(app: &TestApp, date: &str, text: &str) -> String {
    let entry: Value = app
        .invoke(
            "journal_entry_create",
            json!({ "entry": {
                "page_date": date, "text": text, "written_tz": "UTC",
                "written_offset_min": 0, "session_id": "seed",
            }}),
        )
        .unwrap();
    entry["id"].as_str().unwrap().to_owned()
}

pub(crate) fn seed_task(app: &TestApp, title: &str, due: Option<&str>) -> String {
    let when = due.map(|d| json!({ "date": d, "time": null, "local": local_now() }));
    let task: Value = app
        .invoke(
            "todo_task_create",
            json!({ "title": title, "list_id": null, "when": when, "details": null }),
        )
        .unwrap();
    task["id"].as_str().unwrap().to_owned()
}

#[test]
fn a_model_lists_then_reads_entries_and_cites_one_it_read() {
    let app = TestApp::new();
    let id = seed_entry(&app, "2026-10-22", "Long walk, finally rested.");
    let provider = native(vec![
        Reply::calls(&[("journal_list", r#"{"from":"2026-10-18","to":"2026-10-25"}"#)]),
        Reply::calls(&[("journal_read", &format!(r#"{{"id":"{id}"}}"#))]),
        Reply::text(&["You rested on the walk [", &id, "]."]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, "What did I do last week?");
    let run = until_end(&events);

    assert_eq!(
        run.iter().map(label).collect::<Vec<_>>(),
        [
            "runStarted",
            "entrySaved:user",
            "stepStarted",
            "entrySaved:assistant",
            "toolStarted",
            "entrySaved:tool_result",
            "toolEnded",
            "stepStarted",
            "entrySaved:assistant",
            "toolStarted",
            "entrySaved:tool_result",
            "toolEnded",
            "stepStarted",
            "textDelta",
            "textDelta",
            "textDelta",
            "entrySaved:assistant",
            "runEnded",
        ]
    );
    assert_eq!(ended_outcome(&run), RunOutcome::Answered);
    for event in &run {
        assert_eq!(event.run_id, started["run_id"].as_str().unwrap());
    }

    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    let kinds: Vec<&str> = saved.iter().map(|p| p["kind"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        [
            "model_change",
            "user",
            "assistant",
            "tool_result",
            "assistant",
            "tool_result",
            "assistant"
        ]
    );
    assert_eq!(saved[3]["name"], "journal_list");
    assert_eq!(saved[3]["is_error"], false);
    assert!(
        saved[3]["result"]
            .as_str()
            .unwrap()
            .contains("Observation e1")
    );
    let answer = &saved[6];
    assert_eq!(answer["text"], format!("You rested on the walk [{id}]."));
    assert_eq!(answer["sources"][0]["id"], id);
    assert_eq!(answer["sources"][0]["module"], "journal");
    assert!(answer["budget_note"].is_null());
}

#[test]
fn the_six_tools_return_their_fields_with_defaults_paging_and_deleted_rows_skipped() {
    let app = TestApp::new();
    let a = seed_entry(&app, "2026-10-21", "Morning run and coffee.");
    let b = seed_entry(&app, "2026-10-22", "Long walk, finally rested.");
    let gone = seed_entry(&app, "2026-10-23", "A deleted thought about walks.");
    app.invoke::<Value>("journal_entry_delete", json!({ "id": gone }))
        .unwrap();
    let task = seed_task(&app, "Book the dentist", Some("2026-10-27"));
    seed_task(&app, "Renew passport", Some("2026-10-30"));

    let provider = native(vec![
        Reply::calls(&[("journal_list", r#"{"from":"2026-10-20","to":"2026-10-25"}"#)]),
        Reply::calls(&[("journal_search", r#"{"query":"walk"}"#)]),
        Reply::calls(&[("todo_list", r#"{"state":"open","due_to":"2026-10-27"}"#)]),
        Reply::calls(&[("todo_search", r#"{"query":"dentist"}"#)]),
        Reply::calls(&[("todo_read", &format!(r#"{{"id":"{task}","steps":true}}"#))]),
        Reply::calls(&[("journal_read", &format!(r#"{{"id":"{a}","offset":0}}"#))]),
        Reply::text(&["Done."]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();
    let started = ask(&app, "Summarise my week");
    until_end(&events);

    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    let results: Vec<&Value> = saved
        .iter()
        .filter(|p| p["kind"] == "tool_result")
        .collect();
    assert_eq!(results.len(), 6);
    let text = |i: usize| results[i]["result"].as_str().unwrap().to_owned();

    // journal_list: the two live Entries, not the deleted one.
    assert!(text(0).contains(&a) && text(0).contains(&b) && !text(0).contains(&gone));
    assert!(text(0).contains("\"preview\""));
    // journal_search: same matching as the app's Search: the live Entry with "walk".
    assert!(text(1).contains(&b) && !text(1).contains(&gone));
    // todo_list: open Tasks due by the 27th; the Task due on the 30th is left out.
    assert!(text(2).contains(&task) && !text(2).contains("Renew passport"));
    // todo_search and todo_read.
    assert!(text(3).contains(&task));
    assert!(text(4).contains("\"steps\""));
    assert!(text(5).contains("\"next_offset\""));
}

#[test]
fn a_missing_id_and_bad_arguments_are_error_results_and_the_run_goes_on() {
    let app = TestApp::new();
    let provider = native(vec![
        Reply::calls(&[("journal_read", r#"{"id":"nope"}"#)]),
        Reply::calls(&[("journal_list", r#"{"limit":500}"#)]),
        Reply::calls(&[("journal_list", r#"{"sort":"newest"}"#)]),
        Reply::text(&["Nothing found."]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();
    let started = ask(&app, "Read entry nope");
    let run = until_end(&events);
    assert_eq!(ended_outcome(&run), RunOutcome::Answered);

    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    let results: Vec<&Value> = saved
        .iter()
        .filter(|p| p["kind"] == "tool_result")
        .collect();
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|r| r["is_error"] == true));
    assert!(results[0]["result"].as_str().unwrap().contains("No Entry"));
}

#[test]
fn three_invalid_replies_in_a_row_fail_the_run_as_a_bad_response() {
    let app = TestApp::new();
    let bad = || Reply::calls(&[("journal_search", r#"{"query":"x","extra":1}"#)]);
    let provider = native(vec![bad(), bad(), bad()]);
    install(&app, &provider);
    let events = app.ai_events();
    ask(&app, "Look something up");
    let run = until_end(&events);
    assert_eq!(
        ended_outcome(&run),
        RunOutcome::Failed {
            kind: mnemora_harness::Failure::BadResponse,
            message: mnemora_ai::failure_message(mnemora_harness::Failure::BadResponse).to_owned(),
        }
    );
}

#[test]
fn turning_a_module_off_removes_its_tools_and_the_prompt_says_so() {
    let app = TestApp::new();
    app.set_pref("core.disabled_modules", json!(["todo"]));
    let provider = native(vec![Reply::text(&["Only the Journal is open."])]);
    install(&app, &provider);
    let events = app.ai_events();
    ask(&app, "What is overdue?");
    until_end(&events);

    let (tools, system) = sent(&provider, 0);
    let names = names(&tools);
    assert!(names.contains(&"journal_list".to_owned()));
    assert!(names.iter().all(|n| !n.starts_with("todo_")), "{names:?}");
    assert!(system.contains("Todo is turned off"), "{system}");
    assert!(system.contains("journal_list"), "{system}");
}

#[test]
fn a_cited_id_no_tool_returned_gets_a_correction_turn_then_the_answer() {
    let app = TestApp::new();
    let real = seed_entry(&app, "2026-10-22", "Long walk.");
    let made_up = "01990000-0000-7000-8000-000000000000";
    let provider = native(vec![
        Reply::calls(&[("journal_list", "{}")]),
        Reply::text(&["See it [", made_up, "]."]),
        Reply::text(&["See it [", &real, "]."]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();
    let started = ask(&app, "Which entry?");
    let run = until_end(&events);
    assert_eq!(ended_outcome(&run), RunOutcome::Answered);

    let correction = provider.requests()[2]
        .messages
        .iter()
        .any(|m| format!("{m:?}").contains(made_up));
    assert!(correction, "the Model is told about the id it made up");
    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    assert_eq!(saved.last().unwrap()["sources"][0]["id"], real);
}

#[test]
fn an_exact_repeat_is_answered_with_its_earlier_result_and_three_in_a_row_close_the_investigation()
{
    let app = TestApp::new();
    seed_entry(&app, "2026-10-22", "Long walk.");
    let same = || Reply::calls(&[("journal_list", r#"{"from":"2026-10-22","to":"2026-10-22"}"#)]);
    let provider = native(vec![
        Reply::calls(&[("journal_list", r#"{"from":"2026-10-22","to":"2026-10-22"}"#)]),
        same(),
        same(),
        same(),
        Reply::text(&["Enough looking."]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();
    let started = ask(&app, "Repeat yourself");
    until_end(&events);

    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    let repeat = saved
        .iter()
        .find(|p| p["kind"] == "tool_result" && p["is_error"] == true)
        .unwrap();
    assert!(repeat["result"].as_str().unwrap().contains("e1"));
    // After three repeats in a row the Model is asked to answer: its next request has no Tools.
    let (tools, system) = sent(&provider, 4);
    assert!(tools.is_empty());
    assert!(system.contains("answer"), "{system}");
}

#[test]
fn the_twelfth_request_has_no_tools_and_its_answer_says_it_may_be_incomplete() {
    let app = TestApp::new();
    let mut replies: Vec<Reply> = Vec::new();
    for i in 0..11 {
        let from = format!("2026-10-{:02}", 1 + i);
        replies.push(Reply::calls(&[(
            "journal_list",
            &format!(r#"{{"from":"{from}","to":"{from}"}}"#),
        )]));
    }
    replies.push(Reply::text(&["Here is what I found so far."]));
    let provider = native(replies);
    install(&app, &provider);
    let events = app.ai_events();
    let started = ask(&app, "Look at every day");
    let run = until_end(&events);
    assert_eq!(ended_outcome(&run), RunOutcome::Answered);

    let (tools, _) = sent(&provider, 11);
    assert!(tools.is_empty(), "the last step is finish-only");
    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    let answer = saved.last().unwrap();
    assert_eq!(
        answer["budget_note"],
        "Answered after 12 steps; there may be more to find"
    );
}

#[test]
fn a_length_stop_does_not_run_its_calls_and_the_model_is_asked_for_them_again() {
    let app = TestApp::new();
    seed_entry(&app, "2026-10-22", "Long walk.");
    let provider = native(vec![
        Reply::length_calls(&[("journal_list", "{}")]),
        Reply::text(&["Done."]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();
    let started = ask(&app, "List it");
    until_end(&events);

    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    let result = saved.iter().find(|p| p["kind"] == "tool_result").unwrap();
    assert_eq!(result["is_error"], true);
    assert!(result["result"].as_str().unwrap().contains("cut off"));
}

#[test]
fn at_most_eight_calls_run_per_step_and_they_run_in_order() {
    let app = TestApp::new();
    seed_entry(&app, "2026-10-22", "Long walk.");
    let days: Vec<String> = (1..=9)
        .map(|d| format!(r#"{{"from":"2026-10-0{d}","to":"2026-10-0{d}"}}"#))
        .collect();
    let nine: Vec<(&str, &str)> = days
        .iter()
        .map(|args| ("journal_list", args.as_str()))
        .collect();
    let provider = native(vec![Reply::calls(&nine), Reply::text(&["Done."])]);
    install(&app, &provider);
    let events = app.ai_events();
    let started = ask(&app, "Nine lists");
    until_end(&events);

    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    let results: Vec<&Value> = saved
        .iter()
        .filter(|p| p["kind"] == "tool_result")
        .collect();
    assert_eq!(results.len(), 9);
    assert!(results[..8].iter().all(|r| r["is_error"] == false));
    assert_eq!(results[8]["is_error"], true);
    assert!(
        results[8]["result"]
            .as_str()
            .unwrap()
            .contains("at most eight")
    );
}

/// The calls as the scripted Provider takes them, borrowed from `calls`.
fn borrow<'a>(calls: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    calls
        .iter()
        .map(|(name, args)| (*name, args.as_str()))
        .collect()
}

#[test]
fn a_prompt_over_the_budget_ends_the_run_as_too_long() {
    let app = TestApp::new();
    // Each read returns a page of 8 KB, so eight reads a Step fill the 100 KB budget in two Steps.
    let body = "walk ".repeat(3_400);
    let ids: Vec<String> = (1..=8)
        .map(|day| seed_entry(&app, &format!("2026-10-{day:02}"), &body))
        .collect();
    let first: Vec<(&str, String)> = ids
        .iter()
        .map(|id| ("journal_read", format!(r#"{{"id":"{id}"}}"#)))
        .collect();
    let second: Vec<(&str, String)> = ids
        .iter()
        .map(|id| ("journal_read", format!(r#"{{"id":"{id}","offset":8192}}"#)))
        .collect();
    let provider = native(vec![
        Reply::calls(&borrow(&first)),
        Reply::calls(&borrow(&second)),
        Reply::text(&["Never reached."]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();
    ask(&app, "Everything");
    let run = until_end(&events);
    assert_eq!(
        ended_outcome(&run),
        RunOutcome::Failed {
            kind: mnemora_harness::Failure::TooLong,
            message: mnemora_ai::failure_message(mnemora_harness::Failure::TooLong).to_owned(),
        }
    );
    assert_eq!(
        provider.requests().len(),
        2,
        "the third request was never sent"
    );
}

#[test]
fn a_provider_without_native_tools_is_sent_no_tools_and_told_so() {
    let app = TestApp::new();
    let provider = Arc::new(ScriptedProvider::new(
        "Scripted",
        "scripted-model",
        vec![Reply::text(&["No data here."])],
    ));
    install(&app, &provider);
    let events = app.ai_events();
    ask(&app, "Anything?");
    until_end(&events);

    let (tools, system) = sent(&provider, 0);
    assert!(tools.is_empty());
    assert!(system.contains("cannot use Tools"), "{system}");
}

#[test]
fn a_list_pages_with_next_offset_until_the_last_row_and_deleted_rows_never_count() {
    let app = TestApp::new();
    let days = ["2026-10-20", "2026-10-21", "2026-10-22"];
    let ids: Vec<String> = days.iter().map(|d| seed_entry(&app, d, "Walk.")).collect();
    let gone = seed_entry(&app, "2026-10-23", "Walk, deleted.");
    app.invoke::<Value>("journal_entry_delete", json!({ "id": gone }))
        .unwrap();
    let provider = native(vec![
        Reply::calls(&[("journal_list", r#"{"limit":2}"#)]),
        Reply::calls(&[("journal_list", r#"{"limit":2,"offset":2}"#)]),
        Reply::text(&["Three walks."]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();
    let started = ask(&app, "List them");
    until_end(&events);

    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    let pages: Vec<&Value> = saved
        .iter()
        .filter(|p| p["kind"] == "tool_result")
        .collect();
    let first = pages[0]["result"].as_str().unwrap();
    let second = pages[1]["result"].as_str().unwrap();
    assert!(first.contains("\"next_offset\":2"), "{first}");
    assert!(second.contains("\"next_offset\":null"), "{second}");
    assert!(first.contains(&ids[2]) && first.contains(&ids[1]) && !first.contains(&ids[0]));
    assert!(second.contains(&ids[0]) && !second.contains(&gone));
}

/// Stamps a Task as closed at `ms` (epoch milliseconds). Closing through the commands stamps the
/// system clock, so a test places the close directly.
fn close_at(app: &TestApp, id: &str, state: &str, ms: i64) {
    app.with_db(|c| {
        c.execute(
            "UPDATE todo_tasks SET state = ?2, closed_at = ?3 WHERE id = ?1",
            (id, state, ms),
        )
        .unwrap();
    });
}

/// Asks a question as if the device were in `time_zone`.
pub(crate) fn ask_in(app: &TestApp, text: &str, time_zone: &str) -> Value {
    app.invoke(
        "ai_run_start",
        json!({ "request": {
            "conversation_id": null,
            "text": text,
            "provider_id": provider_id(app),
            "model": "scripted-model",
            "local": { "today": TODAY, "weekday": "Sunday", "time_zone": time_zone, "week_start": "mon" },
        } }),
    )
    .unwrap()
}

/// Runs a question on a scripted native-tool Provider and returns each Tool result, in order.
pub(crate) fn tool_results(app: &TestApp, replies: Vec<Reply>) -> Vec<String> {
    tool_results_in(app, "Europe/London", replies)
}

/// As `tool_results`, for a device in `time_zone`.
pub(crate) fn tool_results_in(app: &TestApp, time_zone: &str, replies: Vec<Reply>) -> Vec<String> {
    let provider = native(replies);
    install(app, &provider);
    let events = app.ai_events();
    let started = ask_in(app, "What did I close?", time_zone);
    until_end(&events);
    payloads(app, started["conversation_id"].as_str().unwrap())
        .iter()
        .filter(|p| p["kind"] == "tool_result")
        .map(|p| p["result"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn todo_list_with_a_closed_day_lists_done_and_wont_do_tasks_by_default() {
    let app = TestApp::new();
    let open = seed_task(&app, "Still open", Some("2026-11-05"));
    let early = seed_task(&app, "Closed early", None);
    let late = seed_task(&app, "Closed late", None);
    let declined = seed_task(&app, "Declined in the range", None);
    close_at(&app, &early, "done", 1_792_490_400_000); // 2026-10-20 10:00 UTC
    close_at(&app, &late, "done", 1_792_663_200_000); // 2026-10-22 10:00 UTC
    close_at(&app, &declined, "wont", 1_792_576_800_000); // 2026-10-21 10:00 UTC

    let results = tool_results(
        &app,
        vec![
            Reply::calls(&[(
                "todo_list",
                r#"{"closed_from":"2026-10-19","closed_to":"2026-10-23"}"#,
            )]),
            Reply::text(&["Done."]),
        ],
    );

    assert!(
        results[0].contains(&early) && results[0].contains(&late),
        "{}",
        results[0]
    );
    assert!(results[0].contains(&declined), "{}", results[0]);
    assert!(!results[0].contains(&open), "{}", results[0]);
}

#[test]
fn an_explicit_state_still_wins_over_the_closed_day_default() {
    let app = TestApp::new();
    let done = seed_task(&app, "Done on the day", None);
    let wont = seed_task(&app, "Declined on the day", None);
    close_at(&app, &done, "done", 1_792_490_400_000); // 2026-10-20 10:00 UTC
    close_at(&app, &wont, "wont", 1_792_490_400_000);

    let results = tool_results(
        &app,
        vec![
            Reply::calls(&[(
                "todo_list",
                r#"{"state":"wont","closed_from":"2026-10-19","closed_to":"2026-10-23"}"#,
            )]),
            Reply::text(&["Done."]),
        ],
    );

    assert!(results[0].contains(&wont), "{}", results[0]);
    assert!(!results[0].contains(&done), "{}", results[0]);
}

#[test]
fn a_closed_day_list_is_ordered_by_when_each_task_closed_newest_first() {
    let app = TestApp::new();
    // The Task closed first is due first: a due-date order would list it first too.
    let first = seed_task(&app, "Closed first", Some("2026-11-01"));
    let second = seed_task(&app, "Closed second", Some("2026-11-20"));
    close_at(&app, &first, "done", 1_792_490_400_000); // 2026-10-20 10:00 UTC
    close_at(&app, &second, "done", 1_792_663_200_000); // 2026-10-22 10:00 UTC

    let results = tool_results(
        &app,
        vec![
            Reply::calls(&[(
                "todo_list",
                r#"{"closed_from":"2026-10-19","closed_to":"2026-10-23"}"#,
            )]),
            Reply::text(&["Done."]),
        ],
    );

    let at = |id: &str| results[0].find(id).unwrap_or(usize::MAX);
    assert!(at(&second) < at(&first), "{}", results[0]);
}

/// Two Tasks closed in New York (UTC-4 on 27 October 2026) either side of midnight: 23:50 on the
/// 27th and 00:10 on the 28th. Their UTC dates are both the 28th.
#[test]
fn a_tool_timestamp_comes_with_its_local_date_and_time_in_the_clients_zone() {
    let app = TestApp::new();
    let late = seed_task(&app, "Closed at 23:50 on the 27th", None);
    let after = seed_task(&app, "Closed at 00:10 on the 28th", None);
    close_at(&app, &late, "done", 1_793_159_400_000); // 2026-10-27 23:50 EDT
    close_at(&app, &after, "done", 1_793_160_600_000); // 2026-10-28 00:10 EDT

    let results = tool_results_in(
        &app,
        "America/New_York",
        vec![
            Reply::calls(&[(
                "todo_list",
                r#"{"closed_from":"2026-10-27","closed_to":"2026-10-28"}"#,
            )]),
            Reply::text(&["Done."]),
        ],
    );

    assert!(
        results[0].contains(r#""closed_at":1793159400000"#),
        "{}",
        results[0]
    );
    assert!(
        results[0].contains(r#""closed_at_local":"2026-10-27 23:50""#),
        "{}",
        results[0]
    );
    assert!(
        results[0].contains(r#""closed_at_local":"2026-10-28 00:10""#),
        "{}",
        results[0]
    );
}

/// The 27th in New York holds the 23:50 Task and not the 00:10 one. A UTC day or the machine's own
/// zone (Asia/Kolkata here) would put both on the 28th.
#[test]
fn a_closed_day_is_the_clients_day_in_its_zone_not_utc_or_the_machines() {
    let app = TestApp::new();
    let late = seed_task(&app, "Closed at 23:50 on the 27th", None);
    let after = seed_task(&app, "Closed at 00:10 on the 28th", None);
    close_at(&app, &late, "done", 1_793_159_400_000); // 2026-10-27 23:50 EDT
    close_at(&app, &after, "done", 1_793_160_600_000); // 2026-10-28 00:10 EDT

    let results = tool_results_in(
        &app,
        "America/New_York",
        vec![
            Reply::calls(&[(
                "todo_list",
                r#"{"closed_from":"2026-10-27","closed_to":"2026-10-27"}"#,
            )]),
            Reply::text(&["Done."]),
        ],
    );

    assert!(results[0].contains(&late), "{}", results[0]);
    assert!(!results[0].contains(&after), "{}", results[0]);
}

/// The Task's creation, its Comment and its History line are all at 23:50 on the 27th in New York.
#[test]
fn todo_read_gives_its_creation_comment_and_history_times_a_local_reading() {
    let app = TestApp::new();
    let at = 1_793_159_400_000; // 2026-10-27 23:50 EDT
    app.clock().freeze(at);
    let task = seed_task(&app, "Renew passport", None);
    app.invoke::<Value>(
        "todo_comment_add",
        json!({ "taskId": task, "body": "Photos first." }),
    )
    .unwrap();
    app.with_db(|c| {
        c.execute(
            "UPDATE todo_tasks SET created_at = ?2 WHERE id = ?1",
            (&task, at),
        )
        .unwrap();
        c.execute(
            "UPDATE todo_comments SET created_at = ?2 WHERE task_id = ?1",
            (&task, at),
        )
        .unwrap();
    });

    let results = tool_results_in(
        &app,
        "America/New_York",
        vec![
            Reply::calls(&[(
                "todo_read",
                &format!(r#"{{"id":"{task}","comments":true,"history":true}}"#),
            )]),
            Reply::text(&["Done."]),
        ],
    );

    let body = &results[0];
    assert!(
        body.contains(r#""created_at_local":"2026-10-27 23:50""#),
        "{body}"
    );
    assert!(body.contains(r#""body":"Photos first.""#), "{body}");
    assert!(
        body.contains(r#""created_at":1793159400000,"created_at_local":"2026-10-27 23:50""#),
        "{body}"
    );
    assert!(body.contains(r#""at_local":"2026-10-27 23:50""#), "{body}");
}

/// An Entry written at 23:50 on the 27th in New York reads as that day and time there.
#[test]
fn journal_tools_give_written_at_a_local_reading_in_the_clients_zone() {
    let app = TestApp::new();
    let id = seed_entry(&app, "2026-10-27", "Walked to the river.");
    app.with_db(|c| {
        c.execute(
            "UPDATE journal_entries SET written_at = ?2 WHERE id = ?1",
            (&id, 1_793_159_400_000_i64),
        )
        .unwrap();
    });

    let results = tool_results_in(
        &app,
        "America/New_York",
        vec![
            Reply::calls(&[("journal_list", r#"{"from":"2026-10-27","to":"2026-10-27"}"#)]),
            Reply::calls(&[("journal_read", &format!(r#"{{"id":"{id}"}}"#))]),
            Reply::text(&["Done."]),
        ],
    );

    assert!(
        results[0].contains(r#""written_at":1793159400000"#),
        "{}",
        results[0]
    );
    assert!(
        results[0].contains(r#""written_at_local":"2026-10-27 23:50""#),
        "{}",
        results[0]
    );
    assert!(
        results[1].contains(r#""written_at_local":"2026-10-27 23:50""#),
        "{}",
        results[1]
    );
}

/// A closing-day question in a zone the bundled tz data does not know is refused, not answered in
/// some other zone.
#[test]
fn a_closing_day_question_in_an_unknown_zone_is_refused() {
    let app = TestApp::new();
    let task = seed_task(&app, "Closed on the day", None);
    close_at(&app, &task, "done", 1_793_159_400_000);

    let results = tool_results_in(
        &app,
        "Mars/Olympus",
        vec![
            Reply::calls(&[(
                "todo_list",
                r#"{"closed_from":"2026-10-27","closed_to":"2026-10-27"}"#,
            )]),
            Reply::text(&["Done."]),
        ],
    );

    assert!(
        results[0].contains("could not be read in the device's time zone"),
        "{}",
        results[0]
    );
    assert!(!results[0].contains(&task), "{}", results[0]);
}
