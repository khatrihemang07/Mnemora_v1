//! The `OpenAI`-compatible adapter against recorded wire fixtures: a fake loopback server replies
//! with streamed chunks or errors, so text, Tool calls, stops, keys and failures run with no network.

use mnemora_harness::{
    Call, Failure, Message, Provider, Request, StopReason, StreamEvent, ToolDef, ToolProtocol,
};
use mnemora_providers::CompatProvider;
use mnemora_providers::testing::{Canned, FakeServer};
use serde_json::{Value, json};

const TEXT: &str = include_str!("fixtures/stream_text.sse");
const TOOLS: &str = include_str!("fixtures/stream_tool_calls.sse");
const MODELS: &str = include_str!("fixtures/models.json");
const TOOLS_REFUSED: &str = include_str!("fixtures/error_400_tools.json");

fn request() -> Request {
    Request {
        system: "Be brief.".into(),
        messages: vec![Message::User("What moved?".into())],
        model: "gpt-test".into(),
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

fn provider(address: &str) -> CompatProvider {
    CompatProvider::new("Compatible", "gpt-test", address, None)
}

#[test]
fn it_speaks_native_tools() {
    assert_eq!(
        provider("http://127.0.0.1:1/v1").tool_protocol(),
        ToolProtocol::Native
    );
}

#[test]
fn a_streamed_answer_arrives_as_deltas_in_order_then_done() {
    let server = FakeServer::start(|_| Canned::sse(TEXT));
    assert_eq!(
        run(&provider(&server.address()), &request()),
        vec![
            StreamEvent::TextDelta("1".into()),
            StreamEvent::TextDelta("\n2\n3".into()),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
    let body: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
    assert_eq!(body["stream"], true);
    assert!(
        body.get("tools").is_none(),
        "no tools key when none are offered"
    );
    assert!(body.get("max_tokens").is_none());
}

#[test]
fn streamed_tool_calls_are_whole_when_the_stream_ends_and_keep_their_order() {
    let server = FakeServer::start(|_| Canned::sse(TOOLS));
    assert_eq!(
        run(&provider(&server.address()), &with_tools()),
        vec![
            StreamEvent::TextDelta("Let me look. ".into()),
            StreamEvent::Call(Call {
                id: "call_a1".into(),
                name: "journal_list".into(),
                arguments: r#"{"from":"2026-10-19","to":"2026-10-25"}"#.into(),
                thought_signature: None,
            }),
            StreamEvent::Call(Call {
                id: "call_b2".into(),
                name: "todo_search".into(),
                arguments: r#"{"query":"move"}"#.into(),
                thought_signature: None,
            }),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
}

#[test]
fn tools_are_sent_as_functions_and_the_branch_carries_calls_and_results() {
    let server = FakeServer::start(|_| Canned::sse(TEXT));
    let mut request = with_tools();
    request.messages = vec![
        Message::User("What moved?".into()),
        Message::Assistant {
            text: String::new(),
            calls: vec![Call {
                id: "call_a1".into(),
                name: "journal_list".into(),
                arguments: "{}".into(),
                thought_signature: None,
            }],
        },
        Message::Tool {
            call_id: "call_a1".into(),
            name: "journal_list".into(),
            text: "Observation e1.".into(),
            is_error: false,
        },
        Message::Correction("Cite only what you read.".into()),
    ];
    run(&provider(&server.address()), &request);
    let body: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
    assert_eq!(body["tools"][0]["type"], "function");
    assert_eq!(body["tools"][0]["function"]["name"], "journal_list");
    assert_eq!(
        body["tools"][0]["function"]["description"],
        "Lists Entries by date."
    );
    assert_eq!(
        body["tools"][0]["function"]["parameters"]["properties"]["from"]["type"],
        "string"
    );
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages[2]["role"], "assistant");
    assert!(messages[2]["content"].is_null());
    assert_eq!(messages[2]["tool_calls"][0]["id"], "call_a1");
    assert_eq!(
        messages[2]["tool_calls"][0]["function"]["name"],
        "journal_list"
    );
    assert_eq!(
        messages[3],
        json!({"role": "tool", "tool_call_id": "call_a1", "content": "Observation e1."})
    );
    assert_eq!(
        messages[4],
        json!({"role": "user", "content": "Cite only what you read."})
    );
}

#[test]
fn a_key_is_optional_and_sent_as_a_bearer_token_when_there_is_one() {
    let server = FakeServer::start(|_| Canned::sse(TEXT));
    run(&provider(&server.address()), &request());
    assert_eq!(server.requests()[0].authorization, None);
    let keyed = CompatProvider::new("Keyed", "m", &server.address(), Some("k-9".into()));
    run(&keyed, &request());
    assert_eq!(
        server.requests()[1].authorization.as_deref(),
        Some("Bearer k-9")
    );
}

#[test]
fn extra_fields_are_merged_into_the_request_body() {
    let server = FakeServer::start(|_| Canned::sse(TEXT));
    let capped = provider(&server.address())
        .with_extra(json!({"provider": {"max_price": {"prompt": 0, "completion": 0}}}));
    run(&capped, &request());
    let body: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
    assert_eq!(body["provider"]["max_price"]["prompt"], 0);
}

#[test]
fn a_stream_that_ends_without_a_finish_is_a_bad_response() {
    let cut = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"half\"},\"finish_reason\":null}]}\n\n";
    let server = FakeServer::start(move |_| Canned::sse(cut));
    assert_eq!(
        run(&provider(&server.address()), &request()),
        vec![
            StreamEvent::TextDelta("half".into()),
            StreamEvent::Failed(Failure::BadResponse),
        ]
    );
}

#[test]
fn a_tool_call_with_no_name_fails_the_step_and_emits_no_call() {
    let nameless = "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"x\",\"function\":{\"arguments\":\"{}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n";
    let server = FakeServer::start(move |_| Canned::sse(nameless));
    assert_eq!(
        run(&provider(&server.address()), &with_tools()),
        vec![StreamEvent::Failed(Failure::BadResponse)]
    );
}

#[test]
fn an_error_inside_the_stream_is_a_failure_and_its_text_is_dropped() {
    let stream = "data: {\"error\":{\"message\":\"SECRET-UPSTREAM-42\"}}\n\n";
    let server = FakeServer::start(move |_| Canned::sse(stream));
    let events = run(&provider(&server.address()), &request());
    assert_eq!(events, vec![StreamEvent::Failed(Failure::Unexpected)]);
    assert!(!format!("{events:?}").contains("SECRET"));
}

#[test]
fn a_refused_tools_request_and_a_rejected_key_map_to_their_failures() {
    let server = FakeServer::start(|_| Canned::json(400, TOOLS_REFUSED));
    assert_eq!(
        run(&provider(&server.address()), &with_tools()),
        vec![StreamEvent::Failed(Failure::ModelCannotUseTools)]
    );
    let server = FakeServer::start(|_| Canned::json(401, "{}"));
    assert_eq!(
        run(&provider(&server.address()), &request()),
        vec![StreamEvent::Failed(Failure::KeyRejected)]
    );
}

#[test]
fn the_model_list_comes_from_models() {
    let server = FakeServer::start(|_| Canned::json(200, MODELS));
    assert_eq!(
        provider(&server.address()).list_models(),
        Ok(vec![
            "claude-haiku".into(),
            "claude-sonnet".into(),
            "codex-luna".into()
        ])
    );
}

#[test]
fn a_stop_mid_stream_keeps_the_text_so_far_and_reads_no_further() {
    let server = FakeServer::start(|_| Canned::sse(TEXT));
    let stopped = std::cell::Cell::new(false);
    let mut events = Vec::new();
    provider(&server.address()).stream(&request(), &|| stopped.get(), &mut |event| {
        if matches!(event, StreamEvent::TextDelta(_)) {
            stopped.set(true);
        }
        events.push(event);
    });
    assert_eq!(
        events,
        vec![
            StreamEvent::TextDelta("1".into()),
            StreamEvent::Done(StopReason::Aborted),
        ]
    );
}

#[test]
fn a_stop_before_the_request_sends_nothing() {
    let server = FakeServer::start(|_| Canned::sse(TEXT));
    let mut events = Vec::new();
    provider(&server.address()).stream(&request(), &|| true, &mut |event| events.push(event));
    assert_eq!(events, vec![StreamEvent::Done(StopReason::Aborted)]);
    assert!(server.requests().is_empty(), "no request leaves the device");
}

const GEMINI_TOOLS: &str = include_str!("fixtures/stream_gemini_signature.sse");

#[test]
fn a_gemini_thought_signature_is_read_from_the_stream_and_echoed_on_the_next_step() {
    let server = FakeServer::start(|_| Canned::sse(GEMINI_TOOLS));
    let events = run(&provider(&server.address()), &with_tools());
    assert_eq!(
        events[0],
        StreamEvent::Call(Call {
            id: "call_g1".into(),
            name: "journal_list".into(),
            arguments: r#"{"from":"2026-10-19"}"#.into(),
            thought_signature: Some("sig-abc==".into()),
        })
    );

    let mut next = with_tools();
    next.messages = vec![
        Message::User("What moved?".into()),
        Message::Assistant {
            text: String::new(),
            calls: vec![Call {
                id: "call_g1".into(),
                name: "journal_list".into(),
                arguments: r#"{"from":"2026-10-19"}"#.into(),
                thought_signature: Some("sig-abc==".into()),
            }],
        },
        Message::Tool {
            call_id: "call_g1".into(),
            name: "journal_list".into(),
            text: "Observation e1.".into(),
            is_error: false,
        },
    ];
    run(&provider(&server.address()), &next);
    let body: Value = serde_json::from_str(&server.requests()[1].body).unwrap();
    assert_eq!(
        body["messages"][2]["tool_calls"][0]["extra_content"],
        json!({"google": {"thought_signature": "sig-abc=="}})
    );
}

#[test]
fn a_call_without_a_signature_is_sent_unchanged() {
    let server = FakeServer::start(|_| Canned::sse(TOOLS));
    let events = run(&provider(&server.address()), &with_tools());
    let StreamEvent::Call(first) = &events[1] else {
        panic!("expected a call, got {events:?}");
    };
    assert_eq!(first.thought_signature, None);

    let mut next = with_tools();
    next.messages = vec![Message::Assistant {
        text: String::new(),
        calls: vec![first.clone()],
    }];
    run(&provider(&server.address()), &next);
    let body: Value = serde_json::from_str(&server.requests()[1].body).unwrap();
    assert!(
        body["messages"][1]["tool_calls"][0]
            .get("extra_content")
            .is_none()
    );
}

const GEMINI_PARALLEL: &str = include_str!("fixtures/stream_gemini_parallel.sse");

#[test]
fn gemini_parallel_calls_without_an_index_stay_two_calls_with_their_own_signatures() {
    let server = FakeServer::start(|_| Canned::sse(GEMINI_PARALLEL));
    let events = run(&provider(&server.address()), &with_tools());
    assert_eq!(
        events,
        vec![
            StreamEvent::Call(Call {
                id: "call_t1".into(),
                name: "todo_list".into(),
                arguments: r#"{"due_to":"2026-10-25"}"#.into(),
                thought_signature: Some("sig-todo".into()),
            }),
            StreamEvent::Call(Call {
                id: "call_j2".into(),
                name: "journal_list".into(),
                arguments: r#"{"limit":10}"#.into(),
                thought_signature: Some("sig-journal".into()),
            }),
            StreamEvent::Done(StopReason::Stop),
        ]
    );

    let mut next = with_tools();
    next.messages = vec![Message::Assistant {
        text: String::new(),
        calls: events
            .iter()
            .filter_map(|event| match event {
                StreamEvent::Call(call) => Some(call.clone()),
                _ => None,
            })
            .collect(),
    }];
    run(&provider(&server.address()), &next);
    let body: Value = serde_json::from_str(&server.requests()[1].body).unwrap();
    let calls = &body["messages"][1]["tool_calls"];
    assert_eq!(calls[0]["id"], "call_t1");
    assert_eq!(calls[0]["function"]["name"], "todo_list");
    assert_eq!(
        calls[0]["function"]["arguments"],
        r#"{"due_to":"2026-10-25"}"#
    );
    assert_eq!(
        calls[0]["extra_content"],
        json!({"google": {"thought_signature": "sig-todo"}})
    );
    assert_eq!(calls[1]["id"], "call_j2");
    assert_eq!(calls[1]["function"]["name"], "journal_list");
    assert_eq!(calls[1]["function"]["arguments"], r#"{"limit":10}"#);
    assert_eq!(
        calls[1]["extra_content"],
        json!({"google": {"thought_signature": "sig-journal"}})
    );
}

#[test]
fn one_call_without_an_index_is_one_call_with_its_arguments() {
    let line = "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"id\":\"call_s1\",\"type\":\"function\",\"function\":{\"name\":\"journal_list\",\"arguments\":\"{\\\"from\\\":\\\"2026-10-19\\\"}\"}}]},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n";
    let server = FakeServer::start(move |_| Canned::sse(line));
    assert_eq!(
        run(&provider(&server.address()), &with_tools()),
        vec![
            StreamEvent::Call(Call {
                id: "call_s1".into(),
                name: "journal_list".into(),
                arguments: r#"{"from":"2026-10-19"}"#.into(),
                thought_signature: None,
            }),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
}

#[test]
fn argument_pieces_without_an_index_or_id_continue_the_latest_call() {
    let stream = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"id\":\"call_s1\",\"type\":\"function\",\"function\":{\"name\":\"journal_list\",\"arguments\":\"{\\\"from\\\":\"}}]},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"function\":{\"arguments\":\"\\\"2026-10-19\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"id\":\"call_j2\",\"type\":\"function\",\"function\":{\"name\":\"todo_search\",\"arguments\":\"{\\\"query\\\":\\\"move\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"function\":{\"arguments\":\"\"}}]},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let server = FakeServer::start(move |_| Canned::sse(stream));
    assert_eq!(
        run(&provider(&server.address()), &with_tools()),
        vec![
            StreamEvent::Call(Call {
                id: "call_s1".into(),
                name: "journal_list".into(),
                arguments: r#"{"from":"2026-10-19"}"#.into(),
                thought_signature: None,
            }),
            StreamEvent::Call(Call {
                id: "call_j2".into(),
                name: "todo_search".into(),
                arguments: r#"{"query":"move"}"#.into(),
                thought_signature: None,
            }),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
}

#[test]
fn a_new_id_at_an_index_that_holds_another_id_starts_a_new_call() {
    let stream = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_a1\",\"type\":\"function\",\"function\":{\"name\":\"journal_list\",\"arguments\":\"{}\"}}]},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_b2\",\"type\":\"function\",\"function\":{\"name\":\"todo_search\",\"arguments\":\"{\\\"query\\\":\\\"move\\\"}\"}}]},\"finish_reason\":null}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
        "data: [DONE]\n\n",
    );
    let server = FakeServer::start(move |_| Canned::sse(stream));
    assert_eq!(
        run(&provider(&server.address()), &with_tools()),
        vec![
            StreamEvent::Call(Call {
                id: "call_a1".into(),
                name: "journal_list".into(),
                arguments: "{}".into(),
                thought_signature: None,
            }),
            StreamEvent::Call(Call {
                id: "call_b2".into(),
                name: "todo_search".into(),
                arguments: r#"{"query":"move"}"#.into(),
                thought_signature: None,
            }),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
}
