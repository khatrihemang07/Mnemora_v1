//! Command-layer tests for the AI tracer: a question asked through `ai_run_start` with a scripted
//! Provider, saved as one Conversation, read back with `ai_conversation_get`, with its typed run
//! events in order. They drive the real commands through the mock runtime (ADR 0009).

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use mnemora_ai::{RunEvent, RunOutcome, failure_message};
use mnemora_harness::scripted::{Reply, ScriptedProvider};
use mnemora_harness::{Failure, Message, Provider};
use serde_json::{Value, json};

use crate::AiRunEvent;
use crate::test_support::TestApp;

const TODAY: &str = "2026-10-25";
/// Where the scripted Provider's record points. Never contacted: the scripted adapter answers.
const ADDRESS: &str = "http://127.0.0.1:9/v1";
const TITLE_RULES: &str = include_str!("../../src/lib/ipc/fixtures/ai-title.json");

fn local() -> Value {
    json!({
        "today": TODAY,
        "weekday": "Sunday",
        "time_zone": "Europe/London",
        "week_start": "mon",
    })
}

fn scripted(replies: Vec<Reply>) -> Arc<ScriptedProvider> {
    Arc::new(ScriptedProvider::new("Scripted", "scripted-model", replies))
}

/// Installs the scripted Provider as the one every Run uses, and adds a Provider record for it
/// (the command checks the record, the scripted adapter does the answering).
fn install(app: &TestApp, provider: &Arc<ScriptedProvider>) {
    let provider: Arc<dyn Provider> = provider.clone();
    app.set_ai_provider(provider);
    app.invoke::<Value>(
        "ai_provider_create",
        json!({ "request": { "kind": "wrapper", "name": "Scripted", "address": ADDRESS } }),
    )
    .unwrap();
}

/// The first Provider on the device, or a missing id when there is none.
fn provider_id(app: &TestApp) -> String {
    let list: Value = app.invoke("ai_provider_list", json!({})).unwrap();
    list.as_array()
        .and_then(|providers| providers.first())
        .map_or_else(
            || "missing-provider".to_owned(),
            |p| p["id"].as_str().unwrap().to_owned(),
        )
}

fn ask(app: &TestApp, conversation: Option<&str>, text: &str) -> Result<Value, Value> {
    app.invoke(
        "ai_run_start",
        json!({ "request": {
            "conversation_id": conversation,
            "text": text,
            "provider_id": provider_id(app),
            "model": "scripted-model",
            "local": local(),
        } }),
    )
}

fn get(app: &TestApp, conversation: &str) -> Value {
    app.invoke("ai_conversation_get", json!({ "id": conversation }))
        .unwrap()
}

/// Every event of the Run that was just started, up to and including its end.
pub(crate) fn until_end(events: &Receiver<AiRunEvent>) -> Vec<AiRunEvent> {
    let mut seen = Vec::new();
    loop {
        let event = events
            .recv_timeout(Duration::from_secs(5))
            .expect("the Run ends");
        let ended = matches!(event.event, RunEvent::RunEnded { .. });
        seen.push(event);
        if ended {
            return seen;
        }
    }
}

/// The event's type, with the kind of an entry written: `entrySaved:user`.
pub(crate) fn label(event: &AiRunEvent) -> String {
    let value = serde_json::to_value(&event.event).unwrap();
    let kind = value["type"].as_str().unwrap().to_owned();
    match value.get("kind").and_then(Value::as_str) {
        Some(entry) => format!("{kind}:{entry}"),
        None => kind,
    }
}

pub(crate) fn ended_outcome(run: &[AiRunEvent]) -> RunOutcome {
    match &run.last().expect("a Run has events").event {
        RunEvent::RunEnded { outcome } => outcome.clone(),
        other => panic!("the last event was not the end: {other:?}"),
    }
}

fn conversation_count(app: &TestApp) -> i64 {
    app.with_db(|c| {
        c.query_row("SELECT COUNT(*) FROM ai_conversations", [], |r| r.get(0))
            .unwrap()
    })
}

#[test]
fn a_question_is_answered_and_saved_as_one_conversation_with_its_events_in_order() {
    let app = TestApp::new();
    let provider = scripted(vec![Reply::text(&["Hello ", "there"])]);
    install(&app, &provider);
    let events = app.ai_events();

    let started = ask(&app, None, "What did I write about the move?").unwrap();
    let run = until_end(&events);

    assert_eq!(
        run.iter().map(label).collect::<Vec<_>>(),
        [
            "runStarted",
            "entrySaved:user",
            "stepStarted",
            "textDelta",
            "textDelta",
            "entrySaved:assistant",
            "runEnded",
        ]
    );
    for event in &run {
        assert_eq!(event.run_id, started["run_id"].as_str().unwrap());
        assert_eq!(
            event.conversation_id,
            started["conversation_id"].as_str().unwrap()
        );
    }
    assert_eq!(ended_outcome(&run), RunOutcome::Answered);

    let conversation = get(&app, started["conversation_id"].as_str().unwrap());
    assert_eq!(conversation["title"], "What did I write about the move?");
    assert_eq!(conversation["running"], false);
    // The Model is recorded first, as its own entry, then the question and the answer.
    let entries = conversation["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(
        entries[0]["payload"],
        json!({ "kind": "model_change", "provider_name": "Scripted", "model": "scripted-model" })
    );
    assert_eq!(entries[0]["parent_id"], Value::Null);
    assert_eq!(conversation["provider_name"], "Scripted");
    assert_eq!(conversation["model"], "scripted-model");
    assert_eq!(
        entries[1]["payload"],
        json!({ "kind": "user", "text": "What did I write about the move?" })
    );
    assert_eq!(entries[1]["parent_id"], entries[0]["id"]);
    assert_eq!(entries[2]["parent_id"], entries[1]["id"]);
    assert_eq!(
        entries[2]["payload"],
        json!({
            "kind": "assistant",
            "text": "Hello there",
            "stop": "stop",
            "provider": "Scripted",
            "model": "scripted-model",
        })
    );
}

#[test]
fn a_conversation_is_titled_with_its_first_question_trimmed_and_capped() {
    let cases: Vec<Value> = serde_json::from_str(TITLE_RULES).unwrap();
    for case in cases {
        let app = TestApp::new();
        install(&app, &scripted(vec![Reply::text(&["ok"])]));
        let events = app.ai_events();
        let started = ask(&app, None, case["question"].as_str().unwrap()).unwrap();
        until_end(&events);
        let conversation = get(&app, started["conversation_id"].as_str().unwrap());
        assert_eq!(
            conversation["title"], case["title"],
            "for {}",
            case["question"]
        );
    }
}

#[test]
fn a_second_question_continues_the_branch_and_the_model_is_sent_all_of_it() {
    let app = TestApp::new();
    let provider = scripted(vec![
        Reply::text(&["First answer"]),
        Reply::text(&["Second answer"]),
    ]);
    install(&app, &provider);
    let events = app.ai_events();

    let first = ask(&app, None, "First question").unwrap();
    until_end(&events);
    let conversation_id = first["conversation_id"].as_str().unwrap();
    ask(&app, Some(conversation_id), "Second question").unwrap();
    until_end(&events);

    let entries = get(&app, conversation_id)["entries"]
        .as_array()
        .unwrap()
        .clone();
    // One Model change, then two questions and their answers; the second question keeps the Model.
    assert_eq!(entries.len(), 5);
    assert_eq!(entries[3]["parent_id"], entries[2]["id"]);
    assert_eq!(
        entries[3]["payload"],
        json!({ "kind": "user", "text": "Second question" })
    );
    assert_eq!(
        provider.requests()[1].messages,
        vec![
            Message::User("First question".into()),
            Message::Assistant("First answer".into()),
            Message::User("Second question".into()),
        ]
    );
}

#[test]
fn the_model_is_told_the_date_the_time_zone_and_the_week_start() {
    let app = TestApp::new();
    let provider = scripted(vec![Reply::text(&["ok"])]);
    install(&app, &provider);
    let events = app.ai_events();
    ask(&app, None, "What is overdue?").unwrap();
    until_end(&events);

    let system = &provider.requests()[0].system;
    assert!(
        system.contains(TODAY),
        "the date is missing from the prompt"
    );
    assert!(
        system.contains("Sunday"),
        "the weekday is missing from the prompt"
    );
    assert!(system.contains("Europe/London"), "the time zone is missing");
    assert!(system.contains("Monday"), "the week start is missing");
}

#[test]
fn a_blank_question_is_refused_and_nothing_is_saved() {
    let app = TestApp::new();
    let error = ask(&app, None, " \n\t ").unwrap_err();

    assert_eq!(error["code"], "validation");
    assert_eq!(error["message"], "Type a question first.");
    assert_eq!(conversation_count(&app), 0);
}

#[test]
fn a_failed_step_keeps_the_question_and_says_why_in_one_plain_sentence() {
    let app = TestApp::new();
    install(&app, &scripted(vec![Reply::fail(Failure::Unreachable)]));
    let events = app.ai_events();

    let started = ask(&app, None, "Anything about the move?").unwrap();
    let run = until_end(&events);

    assert_eq!(
        ended_outcome(&run),
        RunOutcome::Failed {
            kind: Failure::Unreachable,
            message: failure_message(Failure::Unreachable).to_owned(),
        }
    );
    let conversation = get(&app, started["conversation_id"].as_str().unwrap());
    assert_eq!(conversation["entries"].as_array().unwrap().len(), 2);
    assert_eq!(conversation["running"], false);
}

#[test]
fn a_question_without_a_provider_is_refused_and_nothing_is_saved() {
    let app = TestApp::new();
    let error = ask(&app, None, "Hello?").unwrap_err();
    assert_eq!(error["code"], "not_found");
    assert_eq!(conversation_count(&app), 0);
}

#[test]
fn a_conversation_that_is_answering_refuses_a_second_question_until_the_first_ends() {
    let app = TestApp::new();
    let (release, gate) = mpsc::channel::<()>();
    install(
        &app,
        &scripted(vec![
            Reply::text(&["Done"]).held(gate),
            Reply::text(&["Later"]),
        ]),
    );
    let events = app.ai_events();

    let started = ask(&app, None, "A slow question").unwrap();
    let conversation_id = started["conversation_id"].as_str().unwrap();
    let busy = ask(&app, Some(conversation_id), "Asked while busy").unwrap_err();
    assert_eq!(busy["code"], "validation");
    assert_eq!(busy["message"], "This Conversation is still answering.");
    assert_eq!(get(&app, conversation_id)["running"], true);

    release.send(()).unwrap();
    until_end(&events);
    assert_eq!(get(&app, conversation_id)["running"], false);

    ask(&app, Some(conversation_id), "Asked after").unwrap();
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
}

#[test]
fn a_question_for_a_conversation_that_is_gone_is_not_found() {
    let app = TestApp::new();
    let error = ask(&app, Some("no-such-conversation"), "Hello?").unwrap_err();

    assert_eq!(error["code"], "not_found");
    assert_eq!(conversation_count(&app), 0);
}
