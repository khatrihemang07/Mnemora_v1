//! Command-layer tests for the wire adapters (#88): the real Wrapper (JSON envelope) and the
//! `OpenAI`-compatible adapter (native Tool calls) ask a fake loopback server, so a Run reads Journal
//! data through the Tools, cites what it read and is corrected when it does not. No network and no
//! Model.

use std::sync::atomic::{AtomicUsize, Ordering};

use mnemora_providers::testing::{Canned, FakeServer};
use serde_json::{Value, json};

use crate::ai_tracer_tests::{ended_outcome, until_end};
use crate::test_support::TestApp;
use mnemora_ai::RunOutcome;

/// A chat reply whose content is one envelope object.
pub(crate) fn reply(envelope: Value) -> String {
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

pub(crate) fn finish(answer: &str, sources: &[&str]) -> Value {
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

pub(crate) fn add_wrapper(app: &TestApp, address: &str) -> String {
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

fn anthropic_reply(content: Value, stop: &str) -> String {
    json!({ "type": "message", "role": "assistant", "content": content, "stop_reason": stop })
        .to_string()
}

#[test]
fn an_anthropic_provider_has_its_fixed_address_and_a_required_key() {
    let app = TestApp::new();
    let created: Value = app
        .invoke(
            "ai_provider_create",
            json!({ "request": { "kind": "anthropic", "name": "Claude", "address": "https://api.anthropic.com/v1/" } }),
        )
        .unwrap();
    assert_eq!(created["address"], "https://api.anthropic.com/v1");
    assert_eq!(
        created["key_state"], "missing",
        "the key is required and none is saved"
    );

    let other = app
        .invoke::<Value>(
            "ai_provider_create",
            json!({ "request": { "kind": "anthropic", "name": "Elsewhere", "address": "https://example.com/v1" } }),
        )
        .unwrap_err();
    assert_eq!(other["code"], "validation");

    let id = created["id"].as_str().unwrap().to_owned();
    let events = app.ai_events();
    let refused = app.invoke::<Value>(
        "ai_run_start",
        json!({ "request": { "conversation_id": null, "text": "Hello?", "provider_id": id,
                             "model": "claude-haiku-5-5", "local": local() } }),
    );
    assert!(refused.is_err(), "no run starts without the key");
    drop(events);

    let set: Value = app
        .invoke(
            "ai_provider_key_set",
            json!({ "id": id, "key": "sk-ant-SECRET-9" }),
        )
        .unwrap();
    assert_eq!(set["key_state"], "saved");
    assert!(!set.to_string().contains("sk-ant-SECRET-9"));
}

#[test]
fn an_anthropic_run_makes_tool_use_calls_with_its_headers_and_the_fixed_limit() {
    let app = TestApp::new();
    let id = seed_entry(&app, "2026-10-20", "Packed the boxes for the move.");
    let turn = AtomicUsize::new(0);
    let first = anthropic_reply(
        json!([{ "type": "tool_use", "id": "toolu_1", "name": "journal_list",
                 "input": { "from": "2026-10-18", "to": "2026-10-25" } }]),
        "tool_use",
    );
    let second = anthropic_reply(
        json!([{ "type": "text", "text": format!("You packed for the move [{id}].") }]),
        "end_turn",
    );
    let server = FakeServer::start(move |_| {
        let reply = if turn.fetch_add(1, Ordering::SeqCst) == 0 {
            &first
        } else {
            &second
        };
        Canned::json(200, reply)
    });
    // The record points at the loopback server; the app's fixed address is checked by the create
    // tests above, and the wire itself by the adapter's tests.
    app.with_db(|conn| {
        conn.execute(
            "INSERT INTO ai_providers (id, name, kind, address, created_at, updated_at) \
             VALUES ('anthropic-test', 'Claude', 'anthropic', ?1, 0, 0)",
            [server.address()],
        )
        .unwrap();
    });
    app.invoke::<Value>(
        "ai_provider_key_set",
        json!({ "id": "anthropic-test", "key": "sk-ant-TEST" }),
    )
    .unwrap();
    let events = app.ai_events();

    let started = ask(&app, "anthropic-test", "What did I do last week?");
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
    let saved = payloads(&app, started["conversation_id"].as_str().unwrap());
    assert_eq!(saved.last().unwrap()["sources"][0]["id"], id);

    let seen = server.requests();
    assert_eq!(seen[0].path, "/v1/messages");
    assert_eq!(
        seen[0]
            .headers
            .iter()
            .find(|(n, _)| n == "x-api-key")
            .map(|(_, v)| v.as_str()),
        Some("sk-ant-TEST")
    );
    for request in &seen {
        let body: Value = serde_json::from_str(&request.body).unwrap();
        assert_eq!(
            body["max_tokens"], 8192,
            "every Anthropic request carries the fixed limit"
        );
    }
    let second_body: Value = serde_json::from_str(&seen[1].body).unwrap();
    let results = second_body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|turn| turn["content"][0]["type"] == "tool_result")
        .expect("the result is sent back as a tool_result block");
    assert_eq!(results["content"][0]["tool_use_id"], "toolu_1");
}

/// An `OpenRouter` record aimed at a loopback server. The app's fixed address is checked by the
/// create test; the wire by the adapter's tests and the requests below.
fn openrouter_at(app: &TestApp, address: &str) -> String {
    app.with_db(|conn| {
        conn.execute(
            "INSERT INTO ai_providers (id, name, kind, address, created_at, updated_at) \
             VALUES ('openrouter-test', 'OpenRouter', 'openrouter', ?1, 0, 0)",
            [address],
        )
        .unwrap();
    });
    app.invoke::<Value>(
        "ai_provider_key_set",
        json!({ "id": "openrouter-test", "key": "sk-or-TEST" }),
    )
    .unwrap();
    "openrouter-test".to_owned()
}

fn openrouter_list() -> String {
    json!({ "data": [
        { "id": "meta-llama/llama-3.3-70b-instruct:free", "pricing": { "prompt": "0", "completion": "0" } },
        { "id": "openai/gpt-4o-mini", "pricing": { "prompt": "0.00000015", "completion": "0.0000006" } },
    ]})
    .to_string()
}

#[test]
fn openrouter_is_created_at_its_fixed_address_with_a_key_required_and_paid_models_off() {
    let app = TestApp::new();
    let created: Value = app
        .invoke(
            "ai_provider_create",
            json!({ "request": { "kind": "openrouter", "name": "OR", "address": "" } }),
        )
        .unwrap();
    assert_eq!(created["address"], "https://openrouter.ai/api/v1");
    assert_eq!(created["key_state"], "missing");
    assert_eq!(created["allow_paid"], false);
    let id = created["id"].as_str().unwrap().to_owned();
    let updated: Value = app
        .invoke(
            "ai_provider_update",
            json!({ "id": id, "request": { "name": "OR", "address": "https://openrouter.ai/api/v1", "allow_paid": true } }),
        )
        .unwrap();
    assert_eq!(updated["allow_paid"], true);
}

#[test]
fn paid_models_are_listed_as_paid_and_refused_while_the_switch_is_off() {
    let app = TestApp::new();
    let server = FakeServer::start(|_| Canned::json(200, &openrouter_list()));
    let id = openrouter_at(&app, &server.address());

    let list: Value = app
        .invoke("ai_models_search", json!({ "providerId": id, "query": "" }))
        .unwrap();
    assert_eq!(list["models"].as_array().unwrap().len(), 2);
    assert_eq!(list["paid_models"], json!(["openai/gpt-4o-mini"]));

    let refused = app.invoke::<Value>(
        "ai_run_start",
        json!({ "request": { "conversation_id": null, "text": "Hi?", "provider_id": id,
                             "model": "openai/gpt-4o-mini", "local": local() } }),
    );
    assert!(
        refused.is_err(),
        "a paid Model cannot be sent while the switch is off"
    );
    assert!(
        server
            .requests()
            .iter()
            .all(|request| request.path.ends_with("/models")),
        "only the Model list was asked, and no chat request was sent"
    );
}

#[test]
fn a_free_route_is_sent_with_the_zero_price_cap_and_a_paid_one_only_once_switched_on() {
    let app = TestApp::new();
    let turn = AtomicUsize::new(0);
    let answer = sse_text("Hello.");
    let server = FakeServer::start(move |req| {
        if req.path.ends_with("/models") {
            Canned::json(200, &openrouter_list())
        } else {
            turn.fetch_add(1, Ordering::SeqCst);
            Canned::sse(&answer)
        }
    });
    let id = openrouter_at(&app, &server.address());
    let events = app.ai_events();

    ask_model(&app, &id, "meta-llama/llama-3.3-70b-instruct:free");
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
    let free_body: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
    assert_eq!(
        free_body["provider"]["max_price"],
        json!({"prompt": 0, "completion": 0})
    );

    // Set through the database: the fixed address would reset this test's address on an update.
    app.with_db(|conn| {
        conn.execute(
            "UPDATE ai_providers SET allow_paid = 1 WHERE id = 'openrouter-test'",
            [],
        )
        .unwrap();
    });
    let events = app.ai_events();
    ask_model(&app, &id, "openai/gpt-4o-mini");
    assert_eq!(ended_outcome(&until_end(&events)), RunOutcome::Answered);
    let paid_body: Value = serde_json::from_str(&server.requests()[1].body).unwrap();
    assert!(
        paid_body.get("provider").is_none(),
        "a paid route carries no price cap"
    );
}

fn ask_model(app: &TestApp, provider_id: &str, model: &str) -> Value {
    app.invoke(
        "ai_run_start",
        json!({ "request": { "conversation_id": null, "text": "Hi?", "provider_id": provider_id,
                             "model": model, "local": local() } }),
    )
    .unwrap()
}
