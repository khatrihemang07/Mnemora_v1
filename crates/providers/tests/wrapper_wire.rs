//! The Wrapper adapter against the wire protocol: a fake loopback server replies with recorded
//! fixtures, so every status, stop reason and stream shape runs with no network and no Model.

use std::time::Duration;

use mnemora_harness::{Failure, Message, Provider, Request, StopReason, StreamEvent};
use mnemora_providers::WrapperProvider;
use mnemora_providers::client::{Client, Options};
use mnemora_providers::testing::{Canned, FakeServer, closed_address};
use serde_json::Value;

const STOP: &str = include_str!("fixtures/chat_stop.json");
const LENGTH: &str = include_str!("fixtures/chat_length.json");
const STREAM: &str = include_str!("fixtures/stream_text.sse");
const MODELS: &str = include_str!("fixtures/models.json");
const TOOLS_REFUSED: &str = include_str!("fixtures/error_400_tools.json");
const NO_SUCH_MODEL: &str = include_str!("fixtures/error_404_model.json");

fn request() -> Request {
    Request {
        system: "Be brief.".into(),
        messages: vec![Message::User("Hi?".into())],
        model: "claude-haiku".into(),
    }
}

/// Every event of one Step, collected.
fn run(provider: &dyn Provider) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    provider.stream(&request(), &mut |event| events.push(event));
    events
}

fn wrapper(address: &str) -> WrapperProvider {
    WrapperProvider::new("Local AI Wrapper", "claude-haiku", address, None)
}

#[test]
fn a_whole_answer_is_one_delta_then_done() {
    let server = FakeServer::start(|_| Canned::json(200, STOP));
    let events = run(&wrapper(&server.address()));
    assert_eq!(
        events,
        vec![
            StreamEvent::TextDelta("Hello there.".into()),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
}

#[test]
fn the_request_is_an_openai_chat_call_without_tools_or_a_key() {
    let server = FakeServer::start(|_| Canned::json(200, STOP));
    run(&wrapper(&server.address()));
    let seen = server.requests();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, "POST");
    assert_eq!(seen[0].path, "/v1/chat/completions");
    assert_eq!(seen[0].authorization, None);
    let body: Value = serde_json::from_str(&seen[0].body).unwrap();
    assert_eq!(body["model"], "claude-haiku");
    assert_eq!(body["stream"], false);
    assert!(body.get("tools").is_none());
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(
        body["messages"][1],
        serde_json::json!({"role": "user", "content": "Hi?"})
    );
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
fn a_length_stop_is_reported_as_length() {
    let server = FakeServer::start(|_| Canned::json(200, LENGTH));
    assert_eq!(
        run(&wrapper(&server.address())),
        vec![
            StreamEvent::TextDelta("The answer was cut".into()),
            StreamEvent::Done(StopReason::Length),
        ]
    );
}

#[test]
fn a_streamed_answer_arrives_as_deltas_in_order() {
    let server = FakeServer::start(|_| Canned::sse(STREAM));
    let events = run(&wrapper(&server.address()).streaming());
    assert_eq!(
        events,
        vec![
            StreamEvent::TextDelta("1".into()),
            StreamEvent::TextDelta("\n2\n3".into()),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
    assert_eq!(
        serde_json::from_str::<Value>(&server.requests()[0].body).unwrap()["stream"],
        true
    );
}

#[test]
fn a_stream_that_ends_without_a_finish_is_a_bad_response() {
    let cut = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"half\"},\"finish_reason\":null}]}\n\n";
    let server = FakeServer::start(move |_| Canned::sse(cut));
    assert_eq!(
        run(&wrapper(&server.address()).streaming()),
        vec![
            StreamEvent::TextDelta("half".into()),
            StreamEvent::Failed(Failure::BadResponse),
        ]
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
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{STOP}",
                STOP.len()
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
        run(&provider),
        vec![
            StreamEvent::TextDelta("Hello there.".into()),
            StreamEvent::Done(StopReason::Stop),
        ]
    );
}
