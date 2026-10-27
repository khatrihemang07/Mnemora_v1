//! The OpenAI-compatible wire format: the chat request, whole and streamed replies, the model
//! list and the error envelope. Pure functions over text, so the fixtures test them directly.

use mnemora_harness::{Failure, Message, Request, StopReason};
use serde_json::{Value, json};

/// Which endpoint a reply came from; the same status means different things on each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    /// `POST /chat/completions`.
    Chat,
    /// `GET /models`.
    Models,
}

/// One thing a streamed chunk says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SseEvent {
    /// A piece of the answer's text.
    Text(String),
    /// The reply is finished and why.
    Finish(StopReason),
    /// The stream is over (`data: [DONE]`).
    Done,
}

/// The JSON body of a chat request. Tools are never offered (no Tool is available yet), and no
/// output limit is sent: the Provider's own default applies.
pub fn chat_body(request: &Request, stream: bool) -> Value {
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    if !request.system.is_empty() {
        messages.push(json!({ "role": "system", "content": request.system }));
    }
    for message in &request.messages {
        messages.push(match message {
            Message::User(text) => json!({ "role": "user", "content": text }),
            Message::Assistant { text, .. } => json!({ "role": "assistant", "content": text }),
            // Tool calls and results travel on the wire only once this adapter offers Tools (#88);
            // until then their text reaches the Model as plain user turns.
            Message::Tool { text, .. } | Message::Correction(text) => {
                json!({ "role": "user", "content": text })
            }
        });
    }
    json!({
        "model": request.model,
        "messages": messages,
        "stream": stream,
    })
}

/// The answer and how it ended, from a whole (non-streamed) chat reply.
pub fn parse_answer(body: &str) -> Result<(String, StopReason), Failure> {
    let value: Value = serde_json::from_str(body).map_err(|_| Failure::BadResponse)?;
    let choice = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .ok_or(Failure::BadResponse)?;
    let text = choice
        .pointer("/message/content")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or(Failure::BadResponse)?;
    let stop = stop_reason(choice.get("finish_reason").and_then(Value::as_str))?
        .ok_or(Failure::BadResponse)?;
    Ok((text.to_owned(), stop))
}

/// The events of one `data:` line of a streamed reply. Blank and comment lines give nothing.
/// An `error` object in the stream is a failure, and its text is dropped.
pub fn sse_events(line: &str) -> Result<Vec<SseEvent>, Failure> {
    let Some(data) = line.strip_prefix("data:") else {
        return Ok(Vec::new());
    };
    let data = data.trim();
    if data == "[DONE]" {
        return Ok(vec![SseEvent::Done]);
    }
    let value: Value = serde_json::from_str(data).map_err(|_| Failure::BadResponse)?;
    if value.get("error").is_some() {
        return Err(Failure::Unexpected);
    }
    let Some(choice) = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
    else {
        // The usage chunk has no choices.
        return Ok(Vec::new());
    };
    let mut events = Vec::new();
    if let Some(text) = choice
        .pointer("/delta/content")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        events.push(SseEvent::Text(text.to_owned()));
    }
    if let Some(stop) = stop_reason(choice.get("finish_reason").and_then(Value::as_str))? {
        events.push(SseEvent::Finish(stop));
    }
    Ok(events)
}

/// How a chat reply ended. `None` for "not yet" (no finish reason on a chunk).
fn stop_reason(reason: Option<&str>) -> Result<Option<StopReason>, Failure> {
    match reason {
        None => Ok(None),
        Some("stop") => Ok(Some(StopReason::Stop)),
        Some("length") => Ok(Some(StopReason::Length)),
        // Tool calls and content filters are not answers Mnemora can show.
        Some(_) => Err(Failure::BadResponse),
    }
}

/// The model ids in a `GET /models` reply.
pub fn parse_model_ids(body: &str) -> Result<Vec<String>, Failure> {
    let value: Value = serde_json::from_str(body).map_err(|_| Failure::BadResponse)?;
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or(Failure::BadResponse)?;
    data.iter()
        .map(|model| {
            model
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .ok_or(Failure::BadResponse)
        })
        .collect()
}

/// The failure for a reply that is not a success. The body is read for its wording only (a
/// tools refusal, a context limit); the text is never kept.
pub fn status_failure(status: u16, body: &str, endpoint: Endpoint) -> Failure {
    let lower = body.to_ascii_lowercase();
    match status {
        400 if lower.contains("tool") => Failure::ModelCannotUseTools,
        400 if lower.contains("context") || lower.contains("too long") => Failure::TooLong,
        401 | 403 => Failure::KeyRejected,
        404 if endpoint == Endpoint::Chat => Failure::ModelUnavailable,
        408 | 504 => Failure::Timeout,
        413 => Failure::TooLong,
        429 => Failure::RateLimited,
        _ => Failure::Unexpected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(system: &str) -> Request {
        Request {
            system: system.into(),
            messages: vec![
                Message::User("first".into()),
                Message::Assistant {
                    text: "answer".into(),
                    calls: Vec::new(),
                },
                Message::User("second".into()),
            ],
            model: "claude-haiku".into(),
            tools: Vec::new(),
        }
    }

    #[test]
    fn the_request_puts_the_system_prompt_first_and_offers_no_tools() {
        let body = chat_body(&request("be brief"), false);
        assert_eq!(body["model"], "claude-haiku");
        assert_eq!(body["stream"], false);
        assert!(body.get("tools").is_none());
        assert!(body.get("max_tokens").is_none());
        assert_eq!(
            body["messages"][0],
            json!({"role": "system", "content": "be brief"})
        );
        assert_eq!(
            body["messages"][3],
            json!({"role": "user", "content": "second"})
        );
        assert_eq!(body["messages"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn an_empty_system_prompt_is_not_sent() {
        let body = chat_body(&request(""), true);
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"][0]["role"], "user");
    }

    #[test]
    fn a_whole_reply_gives_its_text_and_stop_reason() {
        let text = include_str!("../tests/fixtures/chat_stop.json");
        assert_eq!(
            parse_answer(text),
            Ok(("Hello there.".into(), StopReason::Stop))
        );
        let cut = include_str!("../tests/fixtures/chat_length.json");
        assert_eq!(
            parse_answer(cut),
            Ok(("The answer was cut".into(), StopReason::Length))
        );
    }

    #[test]
    fn a_reply_without_text_or_choices_is_a_bad_response() {
        for body in [
            r#"{"choices":[]}"#,
            r#"{"choices":[{"message":{"role":"assistant","content":""},"finish_reason":"stop"}]}"#,
            r#"{"choices":[{"message":{"role":"assistant","content":"x"},"finish_reason":null}]}"#,
            r#"{"choices":[{"message":{"role":"assistant","content":"x"},"finish_reason":"tool_calls"}]}"#,
            "not json",
            "",
        ] {
            assert_eq!(parse_answer(body), Err(Failure::BadResponse), "{body}");
        }
    }

    #[test]
    fn streamed_chunks_give_text_then_the_finish_then_done() {
        let stream = include_str!("../tests/fixtures/stream_text.sse");
        let events: Vec<SseEvent> = stream
            .lines()
            .flat_map(|line| sse_events(line).unwrap())
            .collect();
        assert_eq!(
            events,
            vec![
                SseEvent::Text("1".into()),
                SseEvent::Text("\n2\n3".into()),
                SseEvent::Finish(StopReason::Stop),
                SseEvent::Done,
            ]
        );
    }

    #[test]
    fn a_stream_error_object_is_a_failure_and_its_text_is_dropped() {
        let line = r#"data: {"error":{"message":"secret upstream text"}}"#;
        assert_eq!(sse_events(line), Err(Failure::Unexpected));
        assert_eq!(sse_events(": keep-alive"), Ok(Vec::new()));
        assert_eq!(sse_events("data: {bad"), Err(Failure::BadResponse));
    }

    #[test]
    fn model_ids_come_from_the_list() {
        let list = include_str!("../tests/fixtures/models.json");
        assert_eq!(
            parse_model_ids(list),
            Ok(vec![
                "claude-haiku".into(),
                "claude-sonnet".into(),
                "codex-luna".into()
            ])
        );
        assert_eq!(
            parse_model_ids(r#"{"data":[{"name":"x"}]}"#),
            Err(Failure::BadResponse)
        );
        assert_eq!(parse_model_ids("{}"), Err(Failure::BadResponse));
    }

    #[test]
    fn statuses_map_to_the_failure_kinds_the_person_reads() {
        let tools = include_str!("../tests/fixtures/error_400_tools.json");
        let missing = include_str!("../tests/fixtures/error_404_model.json");
        let cases = [
            (400, tools, Endpoint::Chat, Failure::ModelCannotUseTools),
            (400, "bad field", Endpoint::Chat, Failure::Unexpected),
            (
                400,
                "maximum context length exceeded",
                Endpoint::Chat,
                Failure::TooLong,
            ),
            (401, "", Endpoint::Chat, Failure::KeyRejected),
            (403, "", Endpoint::Models, Failure::KeyRejected),
            (404, missing, Endpoint::Chat, Failure::ModelUnavailable),
            (404, missing, Endpoint::Models, Failure::Unexpected),
            (408, "", Endpoint::Chat, Failure::Timeout),
            (413, "", Endpoint::Chat, Failure::TooLong),
            (429, "", Endpoint::Chat, Failure::RateLimited),
            (
                502,
                "codex login expired",
                Endpoint::Chat,
                Failure::Unexpected,
            ),
            (504, "", Endpoint::Chat, Failure::Timeout),
            (500, "", Endpoint::Models, Failure::Unexpected),
        ];
        for (status, body, endpoint, expected) in cases {
            assert_eq!(status_failure(status, body, endpoint), expected, "{status}");
        }
    }
}
