//! The Wrapper adapter against the wire protocol: a fake loopback server replies with recorded
//! fixtures, so every status, stop reason and stream shape runs with no network and no Model.

use std::time::Duration;

use mnemora_harness::{
    Call, Failure, Message, Provider, Request, StopReason, StreamEvent, ToolDef, ToolProtocol,
};
use mnemora_providers::WrapperProvider;
use mnemora_providers::client::{Client, Options};
use mnemora_providers::envelope::{self, MAX_ARGUMENT_BYTES};
use mnemora_providers::testing::{Canned, FakeServer, closed_address};
use serde_json::Value;

const FINISH: &str = include_str!("fixtures/envelope_finish.json");
const FENCED: &str = include_str!("fixtures/envelope_fenced_finish.json");
const CALLS: &str = include_str!("fixtures/envelope_calls.json");
const CALLS_CUT: &str = include_str!("fixtures/envelope_calls_length.json");
const PROSE: &str = include_str!("fixtures/envelope_plain_text.json");
const CITED: &str = "e1b2c3d4-0000-4000-8000-000000000001";

const STOP: &str = include_str!("fixtures/chat_stop.json");
const MODELS: &str = include_str!("fixtures/models.json");
const TOOLS_REFUSED: &str = include_str!("fixtures/error_400_tools.json");
const NO_SUCH_MODEL: &str = include_str!("fixtures/error_404_model.json");

fn request() -> Request {
    Request {
        system: "Be brief.".into(),
        messages: vec![Message::User("Hi?".into())],
        model: "claude-haiku".into(),
        tools: Vec::new(),
    }
}

/// Every event of one Step, collected.
fn run(provider: &dyn Provider) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    provider.stream(&request(), &|| false, &mut |event| events.push(event));
    events
}

fn wrapper(address: &str) -> WrapperProvider {
    WrapperProvider::new("Local AI Wrapper", "claude-haiku", address, None)
}

#[test]
fn a_key_is_sent_as_a_bearer_token_when_there_is_one() {
    let server = FakeServer::start(|_| Canned::json(200, STOP));
    let keyed = WrapperProvider::new("Keyed", "m", &server.address(), Some("k-123".into()));
    run(&keyed);
    assert_eq!(
        server.requests()[0].authorization.as_deref(),
        Some("Bearer k-123")
    );
}

#[test]
fn an_error_reply_fails_the_step_with_its_kind() {
    let cases = [
        (400, TOOLS_REFUSED, Failure::ModelCannotUseTools),
        (401, "{}", Failure::KeyRejected),
        (404, NO_SUCH_MODEL, Failure::ModelUnavailable),
        (429, "{}", Failure::RateLimited),
        (
            502,
            "{\"error\":{\"message\":\"codex login expired\"}}",
            Failure::Unexpected,
        ),
        (504, "{}", Failure::Timeout),
    ];
    for (status, body, expected) in cases {
        let server = FakeServer::start(move |_| Canned::json(status, body));
        assert_eq!(
            run(&wrapper(&server.address())),
            vec![StreamEvent::Failed(expected)],
            "status {status}"
        );
    }
}

#[test]
fn raw_error_text_never_reaches_the_events() {
    let secret = "SECRET-UPSTREAM-TEXT-42";
    let body = format!("{{\"error\":{{\"message\":\"{secret}\"}}}}");
    let server = FakeServer::start(move |_| Canned::json(502, &body));
    let events = run(&wrapper(&server.address()));
    assert!(!format!("{events:?}").contains(secret));
}

#[test]
fn a_refused_connection_is_unreachable() {
    assert_eq!(
        run(&wrapper(&closed_address())),
        vec![StreamEvent::Failed(Failure::Unreachable)]
    );
}

#[test]
fn a_reply_that_takes_too_long_is_a_timeout() {
    let server = FakeServer::start(|_| Canned::json(200, STOP).delayed(Duration::from_secs(3)));
    let client = Client::with_options(&Options {
        reply: Duration::from_millis(300),
        roots: None,
    });
    let slow = WrapperProvider::with_client(client, "Slow", "m", &server.address(), None);
    assert_eq!(run(&slow), vec![StreamEvent::Failed(Failure::Timeout)]);
}

#[test]
fn the_model_list_comes_from_models_and_a_failure_is_mapped() {
    let server = FakeServer::start(|_| Canned::json(200, MODELS));
    let provider = wrapper(&server.address());
    assert_eq!(
        provider.list_models(),
        Ok(vec![
            "claude-haiku".into(),
            "claude-sonnet".into(),
            "codex-luna".into()
        ])
    );
    assert_eq!(server.requests()[0].method, "GET");
    assert_eq!(server.requests()[0].path, "/v1/models");

    let down = FakeServer::start(|_| Canned::json(401, "{}"));
    assert_eq!(
        wrapper(&down.address()).list_models(),
        Err(Failure::KeyRejected)
    );
    assert_eq!(
        wrapper(&closed_address()).list_models(),
        Err(Failure::Unreachable)
    );
}

#[test]
fn https_is_spoken_with_a_trusted_root() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;

    use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};

    let certified = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = certified.cert.der().to_vec();
    let key_der = certified.signing_key.serialize_der();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![CertificateDer::from(cert_der.clone())],
        PrivatePkcs8KeyDer::from(key_der).into(),
    )
    .unwrap();
    let config = Arc::new(config);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for tcp in listener.incoming().flatten() {
            let conn = rustls::ServerConnection::new(Arc::clone(&config)).unwrap();
            let mut tls = rustls::StreamOwned::new(conn, tcp);
            let mut reader = BufReader::new(&mut tls);
            let mut length = 0usize;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; length];
            let _ = reader.read_exact(&mut body);
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{FINISH}",
                FINISH.len()
            );
            let _ = tls.write_all(reply.as_bytes());
            let _ = tls.flush();
        }
    });

    let client = Client::with_options(&Options {
        reply: Duration::from_secs(10),
        roots: Some(vec![cert_der]),
    });
    let provider = WrapperProvider::with_client(
        client,
        "Secure",
        "claude-haiku",
        &format!("https://localhost:{port}/v1"),
        None,
    );
    assert_eq!(
        run(&provider).last(),
        Some(&StreamEvent::Done(StopReason::Stop))
    );
}

/// A chat reply whose content is `content`, as the Wrapper sends it.
fn reply_with(content: &str, finish: &str) -> String {
    serde_json::json!({
        "choices": [{"index": 0, "message": {"role": "assistant", "content": content}, "finish_reason": finish}]
    })
    .to_string()
}

fn with_tools() -> Request {
    let mut request = request();
    request.tools = vec![ToolDef {
        name: "journal_list".into(),
        about: "Lists Entries by date.".into(),
        schema: r#"{"type":"object"}"#.into(),
    }];
    request
}

fn run_with(provider: &dyn Provider, request: &Request) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    provider.stream(request, &|| false, &mut |event| events.push(event));
    events
}

#[test]
fn the_wrapper_speaks_the_envelope_protocol() {
    assert_eq!(
        wrapper("http://127.0.0.1:1/v1").tool_protocol(),
        ToolProtocol::Envelope
    );
}

#[test]
fn a_finish_reply_gives_its_answer_its_sources_and_done() {
    let server = FakeServer::start(|_| Canned::json(200, FINISH));
    assert_eq!(
        run_with(&wrapper(&server.address()), &with_tools()),
        vec![
            StreamEvent::TextDelta(format!("You wrote about the move [{CITED}].")),
            StreamEvent::Sources(vec![CITED.into()]),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
}

#[test]
fn a_reply_in_a_json_fence_is_read_the_same_way() {
    let server = FakeServer::start(|_| Canned::json(200, FENCED));
    let events = run_with(&wrapper(&server.address()), &with_tools());
    assert_eq!(events.last(), Some(&StreamEvent::Done(StopReason::Stop)));
    assert!(events.contains(&StreamEvent::Sources(vec![CITED.into()])));
}

#[test]
fn a_tools_reply_gives_its_calls_in_order_with_distinct_ids_and_done() {
    let server = FakeServer::start(|_| Canned::json(200, CALLS));
    let events = run_with(&wrapper(&server.address()), &with_tools());
    assert_eq!(
        events,
        vec![
            StreamEvent::Call(Call {
                id: "w1".into(),
                name: "journal_list".into(),
                arguments: r#"{"from":"2026-10-19","to":"2026-10-25"}"#.into(),
            }),
            StreamEvent::Call(Call {
                id: "w2".into(),
                name: "todo_search".into(),
                arguments: r#"{"query":"move"}"#.into(),
            }),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
}

#[test]
fn a_cut_off_tools_reply_keeps_its_calls_and_reports_length_so_the_loop_refuses_them() {
    let server = FakeServer::start(|_| Canned::json(200, CALLS_CUT));
    let events = run_with(&wrapper(&server.address()), &with_tools());
    assert_eq!(events.last(), Some(&StreamEvent::Done(StopReason::Length)));
    assert_eq!(events.len(), 3);
}

#[test]
fn a_reply_that_breaks_the_protocol_is_a_correction_and_the_step_still_ends() {
    let server = FakeServer::start(|_| Canned::json(200, PROSE));
    assert_eq!(
        run_with(&wrapper(&server.address()), &with_tools()),
        vec![
            StreamEvent::Correction(envelope::SHAPE.into()),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
}

#[test]
fn each_protocol_breach_gets_its_own_correction() {
    let long = "a".repeat(MAX_ARGUMENT_BYTES + 1);
    let nine: Vec<String> = (0..9)
        .map(|_| r#"{"name":"journal_list","arguments":"{}"}"#.to_owned())
        .collect();
    let cases: Vec<(String, &str)> = vec![
        (
            r#"{"action":"finish","calls":[{"name":"journal_list","arguments":"{}"}],"answer":"x","sources":[]}"#.into(),
            envelope::FINISH_WITH_CALLS,
        ),
        (
            r#"{"action":"tools","calls":[{"name":"journal_list","arguments":"{}"}],"answer":"x","sources":[]}"#.into(),
            envelope::TOOLS_WITH_ANSWER,
        ),
        (
            r#"{"action":"tools","calls":[],"answer":"","sources":[]}"#.into(),
            envelope::NO_CALLS,
        ),
        (
            r#"{"action":"finish","calls":[],"answer":"  ","sources":[]}"#.into(),
            envelope::EMPTY_ANSWER,
        ),
        (
            format!(r#"{{"action":"tools","calls":[{}],"answer":"","sources":[]}}"#, nine.join(",")),
            envelope::TOO_MANY_CALLS,
        ),
        (
            r#"{"action":"tools","calls":[{"name":"journal_list","arguments":"[1]"}],"answer":"","sources":[]}"#.into(),
            envelope::BAD_ARGUMENTS,
        ),
        (
            format!(r#"{{"action":"tools","calls":[{{"name":"journal_list","arguments":"{{\"q\":\"{long}\"}}"}}],"answer":"","sources":[]}}"#),
            envelope::BAD_ARGUMENTS,
        ),
        (
            r#"{"action":"finish","calls":[],"answer":"x","sources":[],"extra":1}"#.into(),
            envelope::SHAPE,
        ),
        (
            r#"{"action":"skip","calls":[],"answer":"x","sources":[]}"#.into(),
            envelope::SHAPE,
        ),
    ];
    for (content, correction) in cases {
        let server = FakeServer::start(move |_| Canned::json(200, &reply_with(&content, "stop")));
        assert_eq!(
            run_with(&wrapper(&server.address()), &with_tools()),
            vec![
                StreamEvent::Correction(correction.into()),
                StreamEvent::Done(StopReason::Stop),
            ],
            "{correction}"
        );
    }
}

#[test]
fn a_tools_reply_on_a_final_step_is_a_correction() {
    let server = FakeServer::start(|_| Canned::json(200, CALLS));
    assert_eq!(
        run_with(&wrapper(&server.address()), &request()),
        vec![
            StreamEvent::Correction(envelope::NO_TOOLS.into()),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
}

#[test]
fn the_request_carries_the_envelope_as_a_strict_format_with_the_tools_listed_and_no_native_tools() {
    let server = FakeServer::start(|_| Canned::json(200, FINISH));
    run_with(&wrapper(&server.address()), &with_tools());
    let body: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
    assert!(body.get("tools").is_none());
    assert!(body.get("max_tokens").is_none());
    assert_eq!(body["stream"], false);
    assert_eq!(body["response_format"]["type"], "json_schema");
    assert_eq!(body["response_format"]["json_schema"]["strict"], true);
    let schema = &body["response_format"]["json_schema"]["schema"];
    assert_eq!(
        schema["properties"]["action"]["enum"],
        serde_json::json!(["tools", "finish"])
    );
    assert_eq!(schema["properties"]["calls"]["maxItems"], 8);
    assert_eq!(schema["additionalProperties"], false);
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.starts_with("Be brief."));
    assert!(system.contains("Reply with one JSON object only"));
    assert!(system.contains("- journal_list: Lists Entries by date."));
}

#[test]
fn a_final_step_offers_only_finish_and_no_calls() {
    let server = FakeServer::start(|_| Canned::json(200, FINISH));
    run_with(&wrapper(&server.address()), &request());
    let body: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
    let schema = &body["response_format"]["json_schema"]["schema"];
    assert_eq!(
        schema["properties"]["action"]["enum"],
        serde_json::json!(["finish"])
    );
    assert_eq!(schema["properties"]["calls"]["maxItems"], 0);
    assert!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("last reply")
    );
}

#[test]
fn the_branch_reaches_the_model_as_turns_with_its_calls_and_their_results() {
    let server = FakeServer::start(|_| Canned::json(200, FINISH));
    let mut request = with_tools();
    request.messages = vec![
        Message::User("What moved?".into()),
        Message::Assistant {
            text: String::new(),
            calls: vec![Call {
                id: "w1".into(),
                name: "journal_list".into(),
                arguments: "{}".into(),
            }],
        },
        Message::Tool {
            call_id: "w1".into(),
            name: "journal_list".into(),
            text: "Observation e1.".into(),
            is_error: false,
        },
        Message::Tool {
            call_id: "w2".into(),
            name: "nope".into(),
            text: "There is no Tool called nope.".into(),
            is_error: true,
        },
        Message::Correction("Send one object.".into()),
    ];
    run_with(&wrapper(&server.address()), &request);
    let body: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages[2]["role"], "assistant");
    assert!(
        messages[2]["content"]
            .as_str()
            .unwrap()
            .contains(r#""name":"journal_list""#)
    );
    assert_eq!(
        messages[3]["content"],
        "Result of journal_list:\nObservation e1."
    );
    assert_eq!(
        messages[4]["content"],
        "Result of nope (refused):\nThere is no Tool called nope."
    );
    assert_eq!(messages[5]["content"], "Send one object.");
}
