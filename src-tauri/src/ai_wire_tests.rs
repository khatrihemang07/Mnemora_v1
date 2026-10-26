//! Command-layer tests for the wire adapters (#88): the real Wrapper (JSON envelope) and the
//! OpenAI-compatible adapter (native Tool calls) ask a fake loopback server, so a Run reads Journal
//! data through the Tools, cites what it read and is corrected when it does not. No network and no
//! Model.

use std::sync::atomic::{AtomicUsize, Ordering};

use mnemora_providers::testing::{Canned, FakeServer};
use serde_json::{Value, json};

use crate::ai_tracer_tests::{ended_outcome, until_end};
use crate::test_support::TestApp;
use mnemora_ai::RunOutcome;

/// A chat reply whose content is one envelope object.
fn reply(envelope: Value) -> String {
    json!({
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": envelope.to_string() },
            "finish_reason": "stop",
        }]
    })
    .to_string()
}

fn tools(calls: &[(&str, &str)]) -> Value {
    let calls: Vec<Value> = calls
        .iter()
        .map(|(name, arguments)| json!({ "name": name, "arguments": arguments }))
        .collect();
    json!({ "action": "tools", "calls": calls, "answer": "", "sources": [] })
}

fn finish(answer: &str, sources: &[&str]) -> Value {
    json!({ "action": "finish", "calls": [], "answer": answer, "sources": sources })
}

/// A fake Wrapper that plays `replies` in order, then repeats the last one.
fn wrapper_playing(replies: Vec<String>) -> FakeServer {
    let turn = AtomicUsize::new(0);
    FakeServer::start(move |_| {
        let index = turn.fetch_add(1, Ordering::SeqCst).min(replies.len() - 1);
        Canned::json(200, &replies[index])
    })
}

fn local() -> Value {
    json!({
        "today": "2026-10-25",
        "weekday": "Sunday",
        "time_zone": "Europe/London",
        "week_start": "mon",
    })
}

fn add_wrapper(app: &TestApp, address: &str) -> String {
    let created: Value = app
        .invoke(
            "ai_provider_create",
            json!({ "request": { "kind": "wrapper", "name": "Mac", "address": address } }),
        )
        .unwrap();
    created["id"].as_str().unwrap().to_owned()
}

fn seed_entry(app: &TestApp, date: &str, text: &str) -> String {
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

fn ask(app: &TestApp, provider_id: &str, text: &str) -> Value {
    app.invoke(
        "ai_run_start",
        json!({ "request": {
            "conversation_id": null,
            "text": text,
            "provider_id": provider_id,
            "model": "claude-haiku",
            "local": local(),
        } }),
    )
    .unwrap()
}

fn payloads(app: &TestApp, conversation: &str) -> Vec<Value> {
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

#[test]
fn a_wrapper_reads_entries_through_the_envelope_and_cites_one_it_read() {
    let app = TestApp::new();
    let id = seed_entry(&app, "2026-10-20", "Packed the boxes for the move.");
    let server = wrapper_playing(vec![
        reply(tools(&[(
            "journal_list",
            r#"{"from":"2026-10-18","to":"2026-10-25"}"#,
        )])),
        reply(finish(&format!("You packed for the move [{id}]."), &[&id])),
    ]);
    let provider = add_wrapper(&app, &server.address());
    let events = app.ai_events();

    let started = ask(&app, &provider, "What did I do last week?");
    let outcome = ended_outcome(&until_end(&events));
    assert_eq!(outcome, RunOutcome::Answered);

    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    let answer = saved.last().unwrap();
    assert_eq!(answer["sources"][0]["id"], id);
    assert_eq!(answer["sources"][0]["module"], "journal");

    let seen = server.requests();
    assert_eq!(seen.len(), 2);
    let first: Value = serde_json::from_str(&seen[0].body).unwrap();
    assert!(first.get("tools").is_none());
    assert_eq!(first["response_format"]["json_schema"]["strict"], true);
    let system = first["messages"][0]["content"].as_str().unwrap();
    assert!(
        system.contains("- journal_list:"),
        "the Tools are listed in the prompt"
    );
    let second: Value = serde_json::from_str(&seen[1].body).unwrap();
    let turns = second["messages"].as_array().unwrap();
    assert!(
        turns.iter().any(|turn| turn["content"]
            .as_str()
            .is_some_and(|text| text.starts_with("Result of journal_list:"))),
        "the Tool result is read back to the Model as a turn"
    );
}

#[test]
fn a_source_no_tool_returned_is_corrected_through_the_envelope_and_the_answer_follows() {
    let app = TestApp::new();
    let id = seed_entry(&app, "2026-10-20", "Packed the boxes for the move.");
    let unseen = "e1b2c3d4-0000-4000-8000-0000000000aa";
    let server = wrapper_playing(vec![
        reply(finish("Something from before.", &[unseen])),
        reply(tools(&[("journal_read", &format!(r#"{{"id":"{id}"}}"#))])),
        reply(finish(&format!("Packing [{id}]."), &[&id])),
    ]);
    let provider = add_wrapper(&app, &server.address());
    let events = app.ai_events();

    let started = ask(&app, &provider, "What did I pack?");
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);

    let seen = server.requests();
    let second: Value = serde_json::from_str(&seen[1].body).unwrap();
    let correction = second["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|turn| turn["content"].as_str())
        .find(|text| text.contains(unseen))
        .expect("the correction names the unseen source");
    assert!(correction.contains("no Tool returned it in this Run"));

    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    assert_eq!(saved.last().unwrap()["sources"][0]["id"], id);
}

#[test]
fn replies_outside_the_protocol_are_corrected_and_three_in_a_row_fail_the_run() {
    let app = TestApp::new();
    let prose = json!({
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": "Just some prose." }, "finish_reason": "stop" }]
    })
    .to_string();
    let server = wrapper_playing(vec![prose]);
    let provider = add_wrapper(&app, &server.address());
    let events = app.ai_events();

    ask(&app, &provider, "Anything?");
    let outcome = ended_outcome(&until_end(&events));
    assert_eq!(
        outcome,
        RunOutcome::Failed {
            kind: mnemora_harness::Failure::BadResponse,
            message: mnemora_ai::failure_message(mnemora_harness::Failure::BadResponse).to_owned(),
        }
    );
    assert_eq!(
        server.requests().len(),
        3,
        "the Run stops after three invalid replies"
    );
}

#[test]
fn a_repeated_call_through_the_envelope_is_answered_with_its_earlier_result() {
    let app = TestApp::new();
    let id = seed_entry(&app, "2026-10-20", "Packed the boxes for the move.");
    let list = r#"{"from":"2026-10-18","to":"2026-10-25"}"#;
    let server = wrapper_playing(vec![
        reply(tools(&[("journal_list", list)])),
        reply(tools(&[("journal_list", list)])),
        reply(finish(&format!("Packing [{id}]."), &[&id])),
    ]);
    let provider = add_wrapper(&app, &server.address());
    let events = app.ai_events();

    let started = ask(&app, &provider, "What did I do?");
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    let repeat = saved.iter().find(|p| {
        p["kind"] == "tool_result"
            && p["result"]
                .as_str()
                .is_some_and(|t| t.contains("repeats call"))
    });
    assert!(
        repeat.is_some(),
        "the repeat is refused with the earlier observation named"
    );
}

/// A streamed reply that makes one Tool call and finishes with it.
fn sse_call(id: &str, name: &str, arguments: &str) -> String {
    let arguments = serde_json::to_string(arguments).unwrap();
    format!(
        "data: {{\"choices\":[{{\"index\":0,\"delta\":{{\"tool_calls\":[{{\"index\":0,\"id\":\"{id}\",\"type\":\"function\",\"function\":{{\"name\":\"{name}\",\"arguments\":{arguments}}}}}]}},\"finish_reason\":null}}]}}\n\n\
         data: {{\"choices\":[{{\"index\":0,\"delta\":{{}},\"finish_reason\":\"tool_calls\"}}]}}\n\n\
         data: [DONE]\n\n"
    )
}

/// A streamed reply that writes `text` and finishes.
fn sse_text(text: &str) -> String {
    let text = serde_json::to_string(text).unwrap();
    format!(
        "data: {{\"choices\":[{{\"index\":0,\"delta\":{{\"content\":{text}}},\"finish_reason\":null}}]}}\n\n\
         data: {{\"choices\":[{{\"index\":0,\"delta\":{{}},\"finish_reason\":\"stop\"}}]}}\n\n\
         data: [DONE]\n\n"
    )
}

fn add_compatible(app: &TestApp, address: &str) -> String {
    let created: Value = app
        .invoke(
            "ai_provider_create",
            json!({ "request": { "kind": "openai_compatible", "name": "Groq", "address": address } }),
        )
        .unwrap();
    assert_eq!(
        created["key_state"], "not_needed",
        "the key is optional and none is saved"
    );
    created["id"].as_str().unwrap().to_owned()
}

#[test]
fn an_openai_compatible_run_makes_native_calls_reads_the_journal_and_cites_it() {
    let app = TestApp::new();
    let id = seed_entry(&app, "2026-10-20", "Packed the boxes for the move.");
    let server = {
        let turn = AtomicUsize::new(0);
        let first = sse_call(
            "call_1",
            "journal_list",
            r#"{"from":"2026-10-18","to":"2026-10-25"}"#,
        );
        let second = sse_text(&format!("You packed for the move [{id}]."));
        FakeServer::start(move |_| {
            let stream = if turn.fetch_add(1, Ordering::SeqCst) == 0 {
                &first
            } else {
                &second
            };
            Canned::sse(stream)
        })
    };
    let provider = add_compatible(&app, &server.address());
    let events = app.ai_events();

    let started = ask(&app, &provider, "What did I do last week?");
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    assert_eq!(saved.last().unwrap()["sources"][0]["id"], id);

    let seen = server.requests();
    let first_body: Value = serde_json::from_str(&seen[0].body).unwrap();
    assert_eq!(first_body["tools"][0]["type"], "function");
    assert!(
        first_body["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["function"]["name"] == "journal_read")
    );
    let second_body: Value = serde_json::from_str(&seen[1].body).unwrap();
    let tool_turn = second_body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|turn| turn["role"] == "tool")
        .expect("the Tool result is sent back as a tool message");
    assert_eq!(tool_turn["tool_call_id"], "call_1");
}

#[test]
fn a_key_on_an_openai_compatible_provider_is_write_only_and_bound_to_its_address() {
    let app = TestApp::new();
    let server = FakeServer::start(|_| Canned::sse(&sse_text("Hi.")));
    let id = add_compatible(&app, &server.address());
    let set: Value = app
        .invoke(
            "ai_provider_key_set",
            json!({ "id": id, "key": "gsk-SECRET-7" }),
        )
        .unwrap();
    assert_eq!(set["key_state"], "saved");
    assert!(
        !set.to_string().contains("gsk-SECRET-7"),
        "the key is never returned"
    );

    let events = app.ai_events();
    ask(&app, &id, "Hello?");
    ended_outcome(&until_end(&events));
    assert_eq!(
        server.requests()[0].authorization.as_deref(),
        Some("Bearer gsk-SECRET-7")
    );

    let cleared: Value = app
        .invoke("ai_provider_key_clear", json!({ "id": id }))
        .unwrap();
    assert_eq!(cleared["key_state"], "not_needed");
}
