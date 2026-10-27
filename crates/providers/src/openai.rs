//! The `OpenAI`-compatible wire format: the chat request, whole and streamed replies, the model
//! list and the error envelope. Pure functions over text, so the fixtures test them directly.

use mnemora_harness::{Failure, Message, Request, StopReason, ToolDef};
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
    /// A piece of one Tool call: its index in the reply and whatever fields this chunk carries.
    ToolDelta {
        /// Which call in the reply this piece belongs to; `None` when the chunk names none
        /// (Gemini's parallel calls), so the caller decides by id.
        index: Option<usize>,
        /// The call's id, when this chunk carries it.
        id: Option<String>,
        /// The Tool's name, when this chunk carries it.
        name: Option<String>,
        /// A piece of the arguments' JSON text.
        arguments: Option<String>,
        /// Gemini's reasoning token for the call, when this chunk carries it.
        thought_signature: Option<String>,
    },
    /// The reply is finished and why.
    Finish(StopReason),
    /// The stream is over (`data: [DONE]`).
    Done,
}

/// The JSON body of a chat request with native Tools. No `tools` key is sent when none are
/// offered, and no output limit is sent: the Provider's own default applies.
pub fn chat_body(request: &Request, stream: bool) -> Value {
    let mut messages = Vec::with_capacity(request.messages.len() + 1);
    if !request.system.is_empty() {
        messages.push(json!({ "role": "system", "content": request.system }));
    }
    for message in &request.messages {
        messages.push(match message {
            Message::User(text) | Message::Correction(text) => {
                json!({ "role": "user", "content": text })
            }
            Message::Assistant { text, calls } if calls.is_empty() => {
                json!({ "role": "assistant", "content": text })
            }
            Message::Assistant { text, calls } => {
                let calls: Vec<Value> = calls
                    .iter()
                    .map(|call| {
                        let mut wire = json!({
                            "id": call.id,
                            "type": "function",
                            "function": { "name": call.name, "arguments": call.arguments },
                        });
                        // Gemini rejects a later Step that drops its signature: send it back as it came.
                        if let Some(signature) = &call.thought_signature {
                            wire["extra_content"] =
                                json!({ "google": { "thought_signature": signature } });
                        }
                        wire
                    })
                    .collect();
                json!({
                    "role": "assistant",
                    "content": if text.is_empty() { Value::Null } else { json!(text) },
                    "tool_calls": calls,
                })
            }
            Message::Tool { call_id, text, .. } => {
                json!({ "role": "tool", "tool_call_id": call_id, "content": text })
            }
        });
    }
    let mut body = json!({
        "model": request.model,
        "messages": messages,
        "stream": stream,
    });
    if !request.tools.is_empty() {
        body["tools"] = request.tools.iter().map(function_tool).collect();
    }
    body
}

/// One Tool as the chat format offers it: a function with its schema as parameters.
fn function_tool(tool: &ToolDef) -> Value {
    let parameters: Value = serde_json::from_str(&tool.schema).unwrap_or_else(|_| json!({}));
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.about,
            "parameters": parameters,
        },
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
    let finish = choice.get("finish_reason").and_then(Value::as_str);
    // A whole answer the envelope reads must not be a reply that made native Tool calls.
    if matches!(finish, Some("tool_calls" | "function_call")) {
        return Err(Failure::BadResponse);
    }
    let stop = stop_reason(finish)?.ok_or(Failure::BadResponse)?;
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
    for piece in choice
        .pointer("/delta/tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        events.push(SseEvent::ToolDelta {
            index: piece
                .get("index")
                .and_then(Value::as_u64)
                .and_then(|index| usize::try_from(index).ok()),
            id: piece.get("id").and_then(Value::as_str).map(str::to_owned),
            name: piece
                .pointer("/function/name")
                .and_then(Value::as_str)
                .map(str::to_owned),
            arguments: piece
                .pointer("/function/arguments")
                .and_then(Value::as_str)
                .map(str::to_owned),
            thought_signature: piece
                .pointer("/extra_content/google/thought_signature")
                .and_then(Value::as_str)
                .map(str::to_owned),
        });
    }
    if let Some(stop) = stop_reason(choice.get("finish_reason").and_then(Value::as_str))? {
        events.push(SseEvent::Finish(stop));
    }
    Ok(events)
}

/// How a chat reply ended. `None` for "not yet" (no finish reason on a chunk). A reply that
/// ends in Tool calls is a `Stop`: the calls are what it made.
fn stop_reason(reason: Option<&str>) -> Result<Option<StopReason>, Failure> {
    match reason {
        None => Ok(None),
        Some("stop" | "tool_calls" | "function_call") => Ok(Some(StopReason::Stop)),
        Some("length") => Ok(Some(StopReason::Length)),
        // Content filters are not answers Mnemora can show.
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
        // Busy or limited: rate limits, a server error, a gateway or service that is down, and
        // Anthropic's overload status (529).
        429 | 500 | 502 | 503 | 529 => Failure::RateLimited,
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
    fn a_gemini_signature_in_a_tool_delta_is_read_and_echoed_in_a_blocking_request() {
        let line = r#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"c1","type":"function","function":{"name":"journal_list","arguments":""},"extra_content":{"google":{"thought_signature":"sig-1"}}}]},"finish_reason":null}]}"#;
        let events = sse_events(line).unwrap();
        assert_eq!(
            events,
            vec![SseEvent::ToolDelta {
                index: Some(0),
                id: Some("c1".into()),
                name: Some("journal_list".into()),
                arguments: Some(String::new()),
                thought_signature: Some("sig-1".into()),
            }]
        );
        let mut request = request("");
        request.messages = vec![Message::Assistant {
            text: String::new(),
            calls: vec![mnemora_harness::Call {
                id: "c1".into(),
                name: "journal_list".into(),
                arguments: "{}".into(),
                thought_signature: Some("sig-1".into()),
            }],
        }];
        let body = chat_body(&request, false);
        assert_eq!(
            body["messages"][0]["tool_calls"][0]["extra_content"],
            json!({"google": {"thought_signature": "sig-1"}})
        );
    }

    #[test]
    fn a_tool_delta_with_no_index_says_so_and_is_not_filed_under_zero() {
        let line = r#"data: {"choices":[{"index":0,"delta":{"tool_calls":[{"id":"c2","type":"function","function":{"name":"todo_list","arguments":"{}"}}]},"finish_reason":null}]}"#;
        assert_eq!(
            sse_events(line),
            Ok(vec![SseEvent::ToolDelta {
                index: None,
                id: Some("c2".into()),
                name: Some("todo_list".into()),
                arguments: Some("{}".into()),
                thought_signature: None,
            }])
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
            (500, "", Endpoint::Chat, Failure::RateLimited),
            (500, "", Endpoint::Models, Failure::RateLimited),
            (
                502,
                "codex login expired",
                Endpoint::Chat,
                Failure::RateLimited,
            ),
            (503, "high demand", Endpoint::Chat, Failure::RateLimited),
            (503, "", Endpoint::Models, Failure::RateLimited),
            (529, "", Endpoint::Chat, Failure::RateLimited),
            (504, "", Endpoint::Chat, Failure::Timeout),
        ];
        for (status, body, endpoint, expected) in cases {
            assert_eq!(status_failure(status, body, endpoint), expected, "{status}");
        }
    }
}
