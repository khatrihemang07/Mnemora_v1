//! Command-layer tests for AI Providers: managing them, their keys and connection tests, Model
//! search and usage, and every failure a Run can end with. The scripted Provider stands in for the
//! Model where a test needs a particular outcome; the rest run the real Wrapper adapter against an
//! in-process loopback server (no network, no Model).

use std::sync::Arc;

use mnemora_ai::{RunEvent, RunOutcome, failure_message};
use mnemora_harness::Failure;
use mnemora_harness::scripted::{Reply, ScriptedProvider};
use mnemora_providers::testing::{Canned, FakeServer, closed_address};
use serde_json::{Value, json};

use crate::ai_tracer_tests::{ended_outcome, label, until_end};
use crate::test_support::TestApp;

const MODELS: &str = include_str!("../../crates/providers/tests/fixtures/models.json");
const ANSWER: &str = include_str!("../../crates/providers/tests/fixtures/envelope_answer.json");
const SECRET: &str = "SECRET-UPSTREAM-TEXT-42";

fn local() -> Value {
    json!({
        "today": "2026-10-25",
        "weekday": "Sunday",
        "time_zone": "Europe/London",
        "week_start": "mon",
    })
}

fn add(app: &TestApp, name: &str, address: &str) -> Result<Value, Value> {
    app.invoke(
        "ai_provider_create",
        json!({ "request": { "kind": "wrapper", "name": name, "address": address } }),
    )
}

fn add_ok(app: &TestApp, name: &str, address: &str) -> String {
    add(app, name, address).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn ask(app: &TestApp, provider_id: &str, model: &str, text: &str) -> Result<Value, Value> {
    app.invoke(
        "ai_run_start",
        json!({ "request": {
            "conversation_id": null,
            "text": text,
            "provider_id": provider_id,
            "model": model,
            "local": local(),
        } }),
    )
}

/// Asks one question and returns how its Run ended.
fn outcome_of(app: &TestApp, provider_id: &str, model: &str) -> RunOutcome {
    let events = app.ai_events();
    ask(app, provider_id, model, "What did I write?").unwrap();
    ended_outcome(&until_end(&events))
}

#[test]
fn a_wrapper_can_be_added_listed_edited_and_deleted_and_needs_no_key() {
    let app = TestApp::new();
    let id = add_ok(&app, "Mac", "http://127.0.0.1:8080/v1");

    let list: Value = app.invoke("ai_provider_list", json!({})).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["name"], "Mac");
    assert_eq!(list[0]["kind"], "wrapper");
    assert_eq!(list[0]["key_state"], "not_needed");

    let edited: Value = app
        .invoke(
            "ai_provider_update",
            json!({ "id": id, "request": {
                "name": "Home server",
                "address": "http://192.168.1.20:8080/v1/",
            } }),
        )
        .unwrap();
    assert_eq!(edited["name"], "Home server");
    assert_eq!(edited["address"], "http://192.168.1.20:8080/v1");

    app.invoke::<Value>("ai_provider_delete", json!({ "id": id }))
        .unwrap();
    let list: Value = app.invoke("ai_provider_list", json!({})).unwrap();
    assert!(list.as_array().unwrap().is_empty());
    let gone = app
        .invoke::<Value>("ai_provider_delete", json!({ "id": id }))
        .unwrap_err();
    assert_eq!(gone["code"], "not_found");
}

#[test]
fn several_providers_can_exist_on_one_device() {
    let app = TestApp::new();
    add_ok(&app, "Mac", "http://127.0.0.1:8080/v1");
    add_ok(&app, "Server", "https://ai.example.com/v1");
    let list: Value = app.invoke("ai_provider_list", json!({})).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 2);
}

#[test]
fn the_address_rules_are_enforced_when_a_provider_is_added_or_edited() {
    let app = TestApp::new();
    assert!(add(&app, "Mac", "https://ai.example.com/v1").is_ok());
    assert!(add(&app, "Tailnet", "http://mac.tailnet.ts.net:8080/v1").is_ok());

    let plain = add(&app, "Open", "http://8.8.8.8/v1").unwrap_err();
    assert_eq!(plain["code"], "validation");
    assert!(plain["message"].as_str().unwrap().contains("Plain http"));
    assert!(add(&app, "Scheme", "ftp://127.0.0.1/v1").is_err());
    assert!(add(&app, "Bare", "127.0.0.1:8080/v1").is_err());

    let id = add_ok(&app, "Mac", "http://127.0.0.1:8080/v1");
    let edit = app
        .invoke::<Value>(
            "ai_provider_update",
            json!({ "id": id, "request": { "name": "Mac", "address": "http://example.com/v1" } }),
        )
        .unwrap_err();
    assert_eq!(edit["code"], "validation");
}

#[test]
fn names_are_required_and_at_most_forty_characters() {
    let app = TestApp::new();
    assert!(add(&app, "   ", "http://127.0.0.1:8080/v1").is_err());
    assert!(add(&app, &"n".repeat(41), "http://127.0.0.1:8080/v1").is_err());
    assert!(add(&app, &"n".repeat(40), "http://127.0.0.1:8080/v1").is_ok());
}

#[test]
fn a_kind_that_takes_no_key_refuses_one_and_saves_nothing() {
    let app = TestApp::new();
    let id = add_ok(&app, "Mac", "http://127.0.0.1:8080/v1");
    let refused = app
        .invoke::<Value>("ai_provider_key_set", json!({ "id": id, "key": "sk-123" }))
        .unwrap_err();
    assert_eq!(refused["code"], "validation");
    let keys_dir = app.state().paths.config_dir.join("ai-keys");
    assert!(!keys_dir.join(&id).exists());
}

#[test]
fn a_connection_test_reports_the_models_or_the_one_sentence_why_not() {
    let answering = FakeServer::start(|_| Canned::json(200, MODELS));
    let app = TestApp::new();
    let id = add_ok(&app, "Mac", &answering.address());
    let ok: Value = app.invoke("ai_provider_test", json!({ "id": id })).unwrap();
    assert_eq!(ok["connected"], true);
    assert_eq!(ok["models"], 3);
    assert_eq!(ok["message"], "Connected · 3 models");

    let refused = FakeServer::start(|_| Canned::json(401, &format!("{{\"error\":\"{SECRET}\"}}")));
    let id = add_ok(&app, "Locked", &refused.address());
    let rejected: Value = app.invoke("ai_provider_test", json!({ "id": id })).unwrap();
    assert_eq!(rejected["connected"], false);
    assert_eq!(rejected["failure"], "key_rejected");
    assert_eq!(rejected["message"], failure_message(Failure::KeyRejected));

    let nobody = add_ok(&app, "Nowhere", &closed_address());
    let unreachable: Value = app
        .invoke("ai_provider_test", json!({ "id": nobody }))
        .unwrap();
    assert_eq!(unreachable["failure"], "unreachable");
    let shown = format!("{rejected}{unreachable}");
    assert!(
        !shown.contains(SECRET),
        "raw Provider text must never reach the screen"
    );
}

#[test]
fn model_search_asks_the_provider_only_when_called_and_filters_by_text() {
    let server = FakeServer::start(|_| Canned::json(200, MODELS));
    let app = TestApp::new();
    let id = add_ok(&app, "Mac", &server.address());
    assert!(
        server.requests().is_empty(),
        "nothing is fetched in the background"
    );

    let all: Value = app
        .invoke("ai_models_search", json!({ "providerId": id, "query": "" }))
        .unwrap();
    assert_eq!(
        all["models"],
        json!(["claude-haiku", "claude-sonnet", "codex-luna"])
    );
    let some: Value = app
        .invoke(
            "ai_models_search",
            json!({ "providerId": id, "query": "CLAUDE" }),
        )
        .unwrap();
    assert_eq!(some["models"], json!(["claude-haiku", "claude-sonnet"]));
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn a_question_goes_through_the_wrapper_and_the_answer_is_saved_with_its_provider_and_model() {
    let server = FakeServer::start(|_| Canned::json(200, ANSWER));
    let app = TestApp::new();
    let id = add_ok(&app, "Mac", &server.address());
    let events = app.ai_events();

    let started = ask(&app, &id, "claude-haiku", "Say hello").unwrap();
    let run = until_end(&events);
    assert_eq!(
        run.iter().map(label).collect::<Vec<_>>(),
        [
            "runStarted",
            "entrySaved:user",
            "stepStarted",
            "textDelta",
            "entrySaved:assistant",
            "runEnded",
        ]
    );
    assert_eq!(ended_outcome(&run), RunOutcome::Answered);

    let sent: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
    assert_eq!(sent["model"], "claude-haiku");
    assert_eq!(sent["stream"], false);
    assert!(sent.get("tools").is_none());

    let conversation: Value = app
        .invoke(
            "ai_conversation_get",
            json!({ "id": started["conversation_id"] }),
        )
        .unwrap();
    let answer = conversation["entries"].as_array().unwrap().last().unwrap();
    assert_eq!(
        answer["payload"],
        json!({
            "kind": "assistant",
            "text": "Hello there.",
            "stop": "stop",
            "provider": "Mac",
            "model": "claude-haiku",
            "calls": [],
            "sources": [],
            "budget_note": null,
        })
    );
    assert_eq!(conversation["provider_name"], "Mac");
    assert_eq!(conversation["provider_id"], id);
}

#[test]
fn each_failure_ends_the_run_with_its_kind_and_keeps_the_question() {
    let cases: [(u16, String, Failure); 6] = [
        (404, "{}".into(), Failure::ModelUnavailable),
        (429, "{}".into(), Failure::RateLimited),
        (504, "{}".into(), Failure::Timeout),
        (
            502,
            format!("{{\"error\":\"{SECRET}\"}}"),
            Failure::RateLimited,
        ),
        (503, "{}".into(), Failure::RateLimited),
        (
            400,
            "{\"error\":\"tools refused\"}".into(),
            Failure::ModelCannotUseTools,
        ),
    ];
    for (status, body, expected) in cases {
        let server = FakeServer::start(move |_| Canned::json(status, &body));
        let app = TestApp::new();
        let id = add_ok(&app, "Mac", &server.address());
        let events = app.ai_events();
        let started = ask(&app, &id, "claude-haiku", "Anything?").unwrap();
        let run = until_end(&events);
        assert_eq!(
            ended_outcome(&run),
            RunOutcome::Failed {
                kind: expected,
                message: failure_message(expected).to_owned(),
            },
            "status {status}"
        );
        assert!(!format!("{run:?}").contains(SECRET));
        let conversation: Value = app
            .invoke(
                "ai_conversation_get",
                json!({ "id": started["conversation_id"] }),
            )
            .unwrap();
        let last = conversation["entries"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()
            .clone();
        assert_eq!(last["payload"]["kind"], "user");
        assert_eq!(last["payload"]["text"], "Anything?");
    }
}

#[test]
fn a_provider_that_cannot_be_reached_ends_the_run_as_unreachable() {
    let app = TestApp::new();
    let id = add_ok(&app, "Off", &closed_address());
    assert_eq!(
        outcome_of(&app, &id, "claude-haiku"),
        RunOutcome::Failed {
            kind: Failure::Unreachable,
            message: failure_message(Failure::Unreachable).to_owned(),
        }
    );
}

#[test]
fn offline_and_interrupted_answers_are_failures_the_person_can_read() {
    for kind in [Failure::Offline, Failure::Interrupted] {
        let app = TestApp::new();
        app.set_ai_provider(Arc::new(ScriptedProvider::new(
            "Scripted",
            "m",
            vec![Reply::text_then_fail(&["half"], kind)],
        )));
        let id = add_ok(&app, "Scripted", "http://127.0.0.1:9/v1");
        let events = app.ai_events();
        ask(&app, &id, "m", "Hello?").unwrap();
        let run = until_end(&events);
        assert_eq!(
            ended_outcome(&run),
            RunOutcome::Failed {
                kind,
                message: failure_message(kind).to_owned(),
            }
        );
        assert!(
            run.iter()
                .any(|e| matches!(e.event, RunEvent::TextDelta { .. }))
        );
    }
}

#[test]
fn usage_lists_the_latest_and_the_most_used_models_and_drops_deleted_providers() {
    let server = FakeServer::start(|_| Canned::json(200, ANSWER));
    let app = TestApp::new();
    let mac = add_ok(&app, "Mac", &server.address());
    let other = add_ok(&app, "Other", &server.address());
    let events = app.ai_events();
    let questions = [
        (&mac, "claude-haiku"),
        (&mac, "claude-haiku"),
        (&mac, "claude-sonnet"),
        (&other, "codex-luna"),
    ];
    for (provider, model) in questions {
        ask(&app, provider, model, "Question").unwrap();
        until_end(&events);
    }

    let usage: Value = app.invoke("ai_models_usage", json!({})).unwrap();
    assert_eq!(usage["recent"][0]["model"], "codex-luna");
    assert_eq!(usage["recent"][0]["provider_name"], "Other");
    assert_eq!(usage["most_used"][0]["model"], "claude-haiku");
    assert_eq!(usage["most_used"][0]["uses"], 2);

    app.invoke::<Value>("ai_provider_delete", json!({ "id": other }))
        .unwrap();
    let usage: Value = app.invoke("ai_models_usage", json!({})).unwrap();
    assert!(
        usage["recent"]
            .as_array()
            .unwrap()
            .iter()
            .all(|u| u["provider_id"] != other)
    );
}

#[test]
fn a_deleted_provider_keeps_its_conversations_readable() {
    let server = FakeServer::start(|_| Canned::json(200, ANSWER));
    let app = TestApp::new();
    let id = add_ok(&app, "Mac", &server.address());
    let events = app.ai_events();
    let started = ask(&app, &id, "claude-haiku", "Still here?").unwrap();
    until_end(&events);
    app.invoke::<Value>("ai_provider_delete", json!({ "id": id }))
        .unwrap();
    let conversation: Value = app
        .invoke(
            "ai_conversation_get",
            json!({ "id": started["conversation_id"] }),
        )
        .unwrap();
    assert_eq!(conversation["provider_name"], "Mac");
    assert_eq!(conversation["entries"].as_array().unwrap().len(), 3);
}

#[test]
fn a_blank_model_id_is_refused_and_nothing_is_saved() {
    let app = TestApp::new();
    let id = add_ok(&app, "Mac", "http://127.0.0.1:8080/v1");
    let error = ask(&app, &id, "   ", "Hello?").unwrap_err();
    assert_eq!(error["code"], "validation");
    assert_eq!(error["message"], "Choose a Model first.");
    let count: i64 = app.with_db(|c| {
        c.query_row("SELECT COUNT(*) FROM ai_conversations", [], |r| r.get(0))
            .unwrap()
    });
    assert_eq!(count, 0);
}

#[test]
fn the_android_backup_rules_exclude_the_key_folder_of_both_environments() {
    use mnemora_core::env::Environment;
    const FULL: &str = include_str!("../gen/android/app/src/main/res/xml/backup_rules.xml");
    const EXTRACTION: &str =
        include_str!("../gen/android/app/src/main/res/xml/data_extraction_rules.xml");
    for env in [Environment::Production, Environment::Sandbox] {
        let path = format!("{}/config/{}/", env.app_identifier(), crate::AI_KEYS_DIR);
        let exclude = format!("<exclude domain=\"root\" path=\"{path}\" />");
        assert!(FULL.contains(&exclude), "backup_rules.xml lacks {path}");
        assert!(
            EXTRACTION.contains(&exclude),
            "data_extraction_rules.xml lacks {path}"
        );
    }
    let manifest = include_str!("../gen/android/app/src/main/AndroidManifest.xml");
    assert!(manifest.contains("android:fullBackupContent=\"@xml/backup_rules\""));
    assert!(manifest.contains("android:dataExtractionRules=\"@xml/data_extraction_rules\""));
}

#[test]
fn failure_wording_matches_the_shared_fixture_the_browser_mock_uses() {
    use mnemora_harness::Failure;
    const FIXTURE: &str = include_str!("../../src/lib/ipc/fixtures/ai-failures.json");
    let cases: Vec<Value> = serde_json::from_str(FIXTURE).unwrap();
    for case in cases {
        let kind: Failure = serde_json::from_value(case["kind"].clone()).unwrap();
        assert_eq!(failure_message(kind), case["message"].as_str().unwrap());
    }
}

#[test]
fn address_rules_match_the_shared_fixture_the_browser_mock_uses() {
    use mnemora_providers::address::{AddressError, check};
    const FIXTURE: &str = include_str!("../../src/lib/ipc/fixtures/ai-addresses.json");
    let cases: Vec<Value> = serde_json::from_str(FIXTURE).unwrap();
    for case in cases {
        let address = case["address"].as_str().unwrap();
        let result = check(address);
        if case["ok"].as_bool().unwrap() {
            assert!(result.is_ok(), "{address} should be accepted");
        } else {
            let expected = match case["reason"].as_str().unwrap() {
                "plain_http" => AddressError::PlainHttpNotAllowed,
                "scheme" => AddressError::UnsupportedScheme,
                _ => AddressError::Invalid,
            };
            assert_eq!(result, Err(expected), "{address}");
        }
    }
}
