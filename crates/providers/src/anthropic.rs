//! Anthropic's Messages API with native Tool calls. The address is fixed and a key is required. Every
//! request carries the fixed output limit, `MAX_OUTPUT_TOKENS`, and no other kind does. A reply is
//! read whole: its text and its `tool_use` blocks come out in order, and the raw reply is dropped.

use mnemora_harness::{
    Call, Failure, Message, Provider, Request, StopReason, StreamEvent, ToolProtocol,
};
use serde_json::{Value, json};

use crate::client::{Client, read_body};
use crate::openai::{self, Endpoint};

/// The address every Anthropic Provider uses. It is not editable.
pub const ADDRESS: &str = "https://api.anthropic.com/v1";
/// The API version every request names.
pub const API_VERSION: &str = "2023-06-01";
/// The output limit sent on every request.
pub const MAX_OUTPUT_TOKENS: u32 = 8192;

/// A Provider for Anthropic's Messages API.
pub struct AnthropicProvider {
    name: String,
    model: String,
    address: String,
    key: String,
    client: Client,
}

impl AnthropicProvider {
    /// A Provider with the app's client. `address` is [`ADDRESS`] in the app; tests point it at a
    /// loopback server.
    pub fn new(name: &str, model: &str, address: &str, key: &str) -> Self {
        Self::with_client(Client::new(), name, model, address, key)
    }

    /// As [`AnthropicProvider::new`], with the client chosen by the caller.
    pub fn with_client(client: Client, name: &str, model: &str, address: &str, key: &str) -> Self {
        Self {
            name: name.to_owned(),
            model: model.to_owned(),
            address: address.trim_end_matches('/').to_owned(),
            key: key.to_owned(),
            client,
        }
    }

    fn headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("x-api-key", self.key.clone()),
            ("anthropic-version", API_VERSION.to_owned()),
        ]
    }

    /// The Model ids this address offers. Also the connection test: any answer means reachable.
    pub fn list_models(&self) -> Result<Vec<String>, Failure> {
        let response = self.client.get_with(
            &format!("{}/models?limit=1000", self.address),
            &self.headers(),
        )?;
        let status = response.status().as_u16();
        let body = read_body(response)?;
        if !(200..300).contains(&status) {
            return Err(openai::status_failure(status, &body, Endpoint::Models));
        }
        openai::parse_model_ids(&body)
    }

    /// The Messages body for a Step. `system` is its own field; the limit is always set.
    pub fn body(&self, request: &Request) -> Value {
        let mut body = json!({
            "model": request.model,
            "max_tokens": MAX_OUTPUT_TOKENS,
            "messages": turns(&request.messages),
        });
        if !request.system.is_empty() {
            body["system"] = json!(request.system);
        }
        if !request.tools.is_empty() {
            body["tools"] = request
                .tools
                .iter()
                .map(|tool| {
                    let schema: Value = serde_json::from_str(&tool.schema)
                        .unwrap_or_else(|_| json!({"type": "object"}));
                    json!({ "name": tool.name, "description": tool.about, "input_schema": schema })
                })
                .collect();
        }
        body
    }

    fn step(&self, request: &Request, emit: &mut dyn FnMut(StreamEvent)) -> Result<(), Failure> {
        let response = self.client.post_with(
            &format!("{}/messages", self.address),
            &self.body(request),
            &self.headers(),
        )?;
        let status = response.status().as_u16();
        let text = read_body(response)?;
        if !(200..300).contains(&status) {
            return Err(openai::status_failure(status, &text, Endpoint::Chat));
        }
        let (answer, calls, stop) = parse_reply(&text)?;
        if !answer.is_empty() {
            emit(StreamEvent::TextDelta(answer));
        }
        for call in calls {
            emit(StreamEvent::Call(call));
        }
        emit(StreamEvent::Done(stop));
        Ok(())
    }
}

/// The branch as Messages turns. Consecutive turns of one role are joined, because a Step's tool
/// results must sit in one user turn right after the calls they answer.
pub fn turns(messages: &[Message]) -> Vec<Value> {
    let mut turns: Vec<(&str, Vec<Value>)> = Vec::new();
    for message in messages {
        let (role, blocks) = match message {
            Message::User(text) | Message::Correction(text) => {
                ("user", vec![json!({ "type": "text", "text": text })])
            }
            Message::Assistant { text, calls } => {
                let mut blocks = Vec::new();
                if !text.is_empty() {
                    blocks.push(json!({ "type": "text", "text": text }));
                }
                for call in calls {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": call.id,
                        "name": call.name,
                        "input": serde_json::from_str::<Value>(&call.arguments)
                            .unwrap_or_else(|_| json!({})),
                    }));
                }
                ("assistant", blocks)
            }
            Message::Tool {
                call_id,
                text,
                is_error,
                ..
            } => (
                "user",
                vec![json!({
                    "type": "tool_result",
                    "tool_use_id": call_id,
                    "content": text,
                    "is_error": is_error,
                })],
            ),
        };
        match turns.last_mut() {
            Some((last, joined)) if *last == role => joined.extend(blocks),
            _ => turns.push((role, blocks)),
        }
    }
    turns
        .into_iter()
        .map(|(role, content)| json!({ "role": role, "content": content }))
        .collect()
}

/// The text, the Tool calls in order and how the reply ended. A reply that ends for any other
/// reason (a refusal, a paused turn) is not an answer Mnemora can show.
pub fn parse_reply(body: &str) -> Result<(String, Vec<Call>, StopReason), Failure> {
    let value: Value = serde_json::from_str(body).map_err(|_| Failure::BadResponse)?;
    let stop = match value.get("stop_reason").and_then(Value::as_str) {
        Some("end_turn" | "tool_use" | "stop_sequence") => StopReason::Stop,
        Some("max_tokens") => StopReason::Length,
        _ => return Err(Failure::BadResponse),
    };
    let blocks = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or(Failure::BadResponse)?;
    let mut text = String::new();
    let mut calls = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => text.push_str(
                block
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or(Failure::BadResponse)?,
            ),
            Some("tool_use") => {
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or(Failure::BadResponse)?;
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty())
                    .ok_or(Failure::BadResponse)?;
                let input = block.get("input").ok_or(Failure::BadResponse)?;
                calls.push(Call {
                    id: id.to_owned(),
                    name: name.to_owned(),
                    arguments: input.to_string(),
                    thought_signature: None,
                });
            }
            // Thinking and other blocks carry nothing Mnemora shows.
            _ => {}
        }
    }
    Ok((text, calls, stop))
}

impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn tool_protocol(&self) -> ToolProtocol {
        ToolProtocol::Native
    }

    fn stream(
        &self,
        request: &Request,
        halt: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(StreamEvent),
    ) {
        // The reply comes whole, so a Stop can only be seen before the request is sent.
        if halt() {
            emit(StreamEvent::Done(StopReason::Aborted));
            return;
        }
        if let Err(failure) = self.step(request, emit) {
            emit(StreamEvent::Failed(failure));
        }
    }
}
