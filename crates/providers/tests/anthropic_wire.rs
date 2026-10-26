//! The Anthropic adapter against recorded Messages fixtures: a fake loopback server replies with
//! whole replies or errors, so text, tool use, limits, headers and failures run with no network.

use mnemora_harness::{
    Call, Failure, Message, Provider, Request, StopReason, StreamEvent, ToolDef, ToolProtocol,
};
use mnemora_providers::AnthropicProvider;
use mnemora_providers::anthropic::{ADDRESS, MAX_OUTPUT_TOKENS};
use mnemora_providers::testing::{Canned, FakeServer};
use serde_json::Value;

const TEXT: &str = include_str!("fixtures/anthropic_text.json");
const TOOL_USE: &str = include_str!("fixtures/anthropic_tool_use.json");
const CUT: &str = include_str!("fixtures/anthropic_max_tokens.json");
const REFUSAL: &str = include_str!("fixtures/anthropic_refusal.json");
const MODELS: &str = include_str!("fixtures/anthropic_models.json");
const OVERLOADED: &str = include_str!("fixtures/anthropic_error_overloaded.json");
const TOOLS_REFUSED: &str = include_str!("fixtures/error_400_tools.json");

fn request() -> Request {
    Request {
        system: "Be brief.".into(),
        messages: vec![Message::User("Hi?".into())],
        model: "claude-haiku-5-5".into(),
        tools: Vec::new(),
    }
}

fn with_tools() -> Request {
    let mut request = request();
    request.tools = vec![ToolDef {
        name: "journal_list".into(),
        about: "Lists Entries by date.".into(),
        schema: r#"{"type":"object","properties":{"from":{"type":"string"}}}"#.into(),
    }];
    request
}

fn run(provider: &dyn Provider, request: &Request) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    provider.stream(request, &|| false, &mut |event| events.push(event));
    events
}

/// A Provider aimed at a loopback server, as the app aims it at the fixed address.
fn provider(address: &str) -> AnthropicProvider {
    AnthropicProvider::new("Claude", "claude-haiku-5-5", address, "sk-ant-TEST")
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

#[test]
fn the_fixed_address_and_the_output_limit_are_the_app_values() {
    assert_eq!(ADDRESS, "https://api.anthropic.com/v1");
    assert_eq!(MAX_OUTPUT_TOKENS, 8192);
    assert_eq!(
        provider("http://127.0.0.1:1/v1").tool_protocol(),
        ToolProtocol::Native
    );
}

#[test]
fn a_text_reply_is_one_delta_then_done() {
    let server = FakeServer::start(|_| Canned::json(200, TEXT));
    assert_eq!(
        run(&provider(&server.address()), &request()),
        vec![
            StreamEvent::TextDelta("Hello there.".into()),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
}

#[test]
fn a_tool_use_reply_gives_its_text_then_its_call_with_arguments_as_text() {
    let server = FakeServer::start(|_| Canned::json(200, TOOL_USE));
    assert_eq!(
        run(&provider(&server.address()), &with_tools()),
        vec![
            StreamEvent::TextDelta("Let me check the Journal.".into()),
            StreamEvent::Call(Call {
                id: "toolu_01A".into(),
                name: "journal_list".into(),
                arguments: r#"{"from":"2026-10-18","to":"2026-10-25"}"#.into(),
                thought_signature: None,
            }),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
}

#[test]
fn a_reply_cut_at_the_output_limit_is_a_length_stop() {
    let server = FakeServer::start(|_| Canned::json(200, CUT));
    assert_eq!(
        run(&provider(&server.address()), &request()),
        vec![
            StreamEvent::TextDelta("The answer was cut".into()),
            StreamEvent::Done(StopReason::Length),
        ]
    );
}

#[test]
fn a_refusal_is_not_an_answer_and_fails_the_step() {
    let server = FakeServer::start(|_| Canned::json(200, REFUSAL));
    assert_eq!(
        run(&provider(&server.address()), &request()),
        vec![StreamEvent::Failed(Failure::BadResponse)]
    );
}

#[test]
fn every_request_carries_the_key_the_version_and_the_fixed_limit() {
    let server = FakeServer::start(|_| Canned::json(200, TEXT));
    run(&provider(&server.address()), &request());
    let seen = &server.requests()[0];
    assert_eq!(seen.path, "/v1/messages");
    assert_eq!(header(&seen.headers, "x-api-key"), Some("sk-ant-TEST"));
    assert_eq!(
        header(&seen.headers, "anthropic-version"),
        Some("2023-06-01")
    );
    assert_eq!(
        seen.authorization, None,
        "Anthropic takes its key in x-api-key, not as a bearer"
    );
    let body: Value = serde_json::from_str(&seen.body).unwrap();
    assert_eq!(body["max_tokens"], 8192);
    assert_eq!(body["system"], "Be brief.");
    assert_eq!(
        body["messages"][0],
        serde_json::json!({"role": "user", "content": [{"type": "text", "text": "Hi?"}]})
    );
    assert!(
        body.get("tools").is_none(),
        "no tools key when none are offered"
    );
}

#[test]
fn tools_are_sent_with_their_schema_and_the_branch_is_joined_into_turns() {
    let server = FakeServer::start(|_| Canned::json(200, TEXT));
    let mut request = with_tools();
    request.messages = vec![
        Message::User("What moved?".into()),
        Message::Assistant {
            text: "Checking.".into(),
            calls: vec![
                Call {
                    id: "toolu_1".into(),
                    name: "journal_list".into(),
                    arguments: r#"{"from":"2026-10-18"}"#.into(),
                    thought_signature: None,
                },
                Call {
                    id: "toolu_2".into(),
                    name: "journal_list".into(),
                    arguments: "{}".into(),
                    thought_signature: None,
                },
            ],
        },
        Message::Tool {
            call_id: "toolu_1".into(),
            name: "journal_list".into(),
            text: "Observation e1.".into(),
            is_error: false,
        },
        Message::Tool {
            call_id: "toolu_2".into(),
            name: "journal_list".into(),
            text: "There is no Tool called x.".into(),
            is_error: true,
        },
    ];
    run(&provider(&server.address()), &request);
    let body: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
    assert_eq!(body["tools"][0]["name"], "journal_list");
    assert_eq!(body["tools"][0]["description"], "Lists Entries by date.");
    assert_eq!(
        body["tools"][0]["input_schema"]["properties"]["from"]["type"],
        "string"
    );
    let turns = body["messages"].as_array().unwrap();
    assert_eq!(turns.len(), 3, "the two tool results share one user turn");
    assert_eq!(turns[1]["role"], "assistant");
    assert_eq!(turns[1]["content"][1]["type"], "tool_use");
    assert_eq!(
        turns[1]["content"][1]["input"],
        serde_json::json!({"from": "2026-10-18"})
    );
    assert_eq!(turns[2]["role"], "user");
    assert_eq!(turns[2]["content"][0]["type"], "tool_result");
    assert_eq!(turns[2]["content"][0]["tool_use_id"], "toolu_1");
    assert_eq!(turns[2]["content"][1]["is_error"], true);
}

#[test]
fn statuses_map_to_the_failures_the_person_reads() {
    let cases = [
        (401, "{}", Failure::KeyRejected),
        (400, TOOLS_REFUSED, Failure::ModelCannotUseTools),
        (404, "{}", Failure::ModelUnavailable),
        (429, "{}", Failure::RateLimited),
        (529, OVERLOADED, Failure::RateLimited),
    ];
    for (status, body, expected) in cases {
        let server = FakeServer::start(move |_| Canned::json(status, body));
        assert_eq!(
            run(&provider(&server.address()), &request()),
            vec![StreamEvent::Failed(expected)],
            "{status}"
        );
    }
}

#[test]
fn the_model_list_is_asked_with_the_same_headers() {
    let server = FakeServer::start(|_| Canned::json(200, MODELS));
    assert_eq!(
        provider(&server.address()).list_models(),
        Ok(vec!["claude-haiku-5-5".into(), "claude-sonnet-5-5".into()])
    );
    let seen = &server.requests()[0];
    assert_eq!(header(&seen.headers, "x-api-key"), Some("sk-ant-TEST"));
    assert_eq!(
        header(&seen.headers, "anthropic-version"),
        Some("2023-06-01")
    );
}
