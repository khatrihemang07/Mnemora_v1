//! Providers that speak the `OpenAI` chat format with native Tool calls: an `OpenAI`-compatible
//! endpoint (a custom address, a key optional) and `OpenRouter` (its fixed address, a key, and its
//! price cap as an extra). Replies stream: text arrives as it is written, and each Tool call is
//! complete when the stream ends. Neither the raw reply nor the service's error text is kept.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, ErrorKind};

use mnemora_harness::{Call, Failure, Provider, Request, StopReason, StreamEvent, ToolProtocol};
use serde_json::Value;

use crate::client::{Client, read_body};
use crate::openai::{self, Endpoint, SseEvent};

/// A Provider for any service that speaks the chat format with Tool calls.
pub struct CompatProvider {
    name: String,
    model: String,
    address: String,
    key: Option<String>,
    client: Client,
    /// Fields merged into every chat request (`OpenRouter`'s price cap, for one).
    extra: Option<Value>,
}

/// One Tool call as it arrives in pieces.
#[derive(Default)]
struct Partial {
    id: Option<String>,
    name: String,
    arguments: String,
    thought_signature: Option<String>,
}

impl CompatProvider {
    /// A Provider with the app's client. `address` already includes the version path.
    pub fn new(name: &str, model: &str, address: &str, key: Option<String>) -> Self {
        Self::with_client(Client::new(), name, model, address, key)
    }

    /// As [`CompatProvider::new`], with the client chosen by the caller.
    pub fn with_client(
        client: Client,
        name: &str,
        model: &str,
        address: &str,
        key: Option<String>,
    ) -> Self {
        Self {
            name: name.to_owned(),
            model: model.to_owned(),
            address: address.trim_end_matches('/').to_owned(),
            key,
            client,
            extra: None,
        }
    }

    /// Merges `extra` into every chat request's body.
    #[must_use]
    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = Some(extra);
        self
    }

    /// The Model ids this address offers. Also the connection test: any answer means reachable.
    pub fn list_models(&self) -> Result<Vec<String>, Failure> {
        let response = self
            .client
            .get(&format!("{}/models", self.address), self.key.as_deref())?;
        let status = response.status().as_u16();
        let body = read_body(response)?;
        if !(200..300).contains(&status) {
            return Err(openai::status_failure(status, &body, Endpoint::Models));
        }
        openai::parse_model_ids(&body)
    }

    /// The chat body for a Step, with the extra fields merged in.
    fn body(&self, request: &Request) -> Value {
        let mut body = openai::chat_body(request, true);
        if let (Some(Value::Object(extra)), Some(map)) = (&self.extra, body.as_object_mut()) {
            for (key, value) in extra {
                map.insert(key.clone(), value.clone());
            }
        }
        body
    }

    fn streamed(
        &self,
        request: &Request,
        halt: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(StreamEvent),
    ) -> Result<(), Failure> {
        let response = self.client.post_json(
            &format!("{}/chat/completions", self.address),
            &self.body(request),
            self.key.as_deref(),
        )?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            let text = read_body(response)?;
            return Err(openai::status_failure(status, &text, Endpoint::Chat));
        }
        let mut reader = BufReader::new(response.into_body().into_reader());
        let mut stop: Option<StopReason> = None;
        let mut calls = Calls::default();
        let mut line = String::new();
        loop {
            // Stopped: the calls still being read are dropped, and the text so far is the answer.
            if halt() {
                emit(StreamEvent::Done(StopReason::Aborted));
                return Ok(());
            }
            line.clear();
            let read = reader.read_line(&mut line).map_err(stream_failure)?;
            if read == 0 {
                break;
            }
            for event in openai::sse_events(line.trim_end())? {
                match event {
                    SseEvent::Text(text) => emit(StreamEvent::TextDelta(text)),
                    SseEvent::ToolDelta {
                        index,
                        id,
                        name,
                        arguments,
                        thought_signature,
                    } => calls.piece(index, id, name, arguments, thought_signature),
                    SseEvent::Finish(reason) => stop = Some(reason),
                    SseEvent::Done => return finish(stop, calls.made, emit),
                }
            }
        }
        // The stream ended without `[DONE]`: an answer only counts once it has finished.
        finish(stop, calls.made, emit)
    }
}

/// The Tool calls of one reply, pieced together as their chunks arrive. A piece with an index
/// continues the call that index is filling. A piece with no index continues the latest call
/// only when it has no id; Gemini sends parallel calls with no index, each with its own id.
#[derive(Default)]
struct Calls {
    made: Vec<Partial>,
    /// The position in `made` of the call each index is filling now.
    filling: HashMap<usize, usize>,
}

impl Calls {
    fn piece(
        &mut self,
        index: Option<usize>,
        id: Option<String>,
        name: Option<String>,
        arguments: Option<String>,
        thought_signature: Option<String>,
    ) {
        let target = match index {
            Some(index) => self.filling.get(&index).copied(),
            None => self.made.len().checked_sub(1),
        };
        let continues = target.filter(|&at| match (&id, &self.made[at].id) {
            // No id: a continuation of whatever the target is.
            (None, _) => true,
            // Indexed, the same call (or one whose id has not arrived yet) keeps going.
            (Some(id), known) if index.is_some() => known.as_ref().is_none_or(|known| known == id),
            // With no index, only a repeat of the latest call's id keeps going.
            (Some(id), known) => known.as_ref() == Some(id),
        });
        let at = continues.unwrap_or_else(|| {
            self.made.push(Partial::default());
            let at = self.made.len() - 1;
            if let Some(index) = index {
                self.filling.insert(index, at);
            }
            at
        });
        let partial = &mut self.made[at];
        if id.is_some() {
            partial.id = id;
        }
        if thought_signature.is_some() {
            partial.thought_signature = thought_signature;
        }
        if let Some(name) = name {
            partial.name.push_str(&name);
        }
        if let Some(arguments) = arguments {
            partial.arguments.push_str(&arguments);
        }
    }
}

/// Ends a stream: a finish reason is needed for the answer to count. Each call is complete now.
fn finish(
    stop: Option<StopReason>,
    calls: Vec<Partial>,
    emit: &mut dyn FnMut(StreamEvent),
) -> Result<(), Failure> {
    let Some(stop) = stop else {
        return Err(Failure::BadResponse);
    };
    let mut made = Vec::with_capacity(calls.len());
    for (position, partial) in calls.into_iter().enumerate() {
        if partial.name.is_empty() {
            return Err(Failure::BadResponse);
        }
        made.push(Call {
            id: partial.id.unwrap_or_else(|| format!("call-{position}")),
            name: partial.name,
            arguments: if partial.arguments.is_empty() {
                "{}".to_owned()
            } else {
                partial.arguments
            },
            thought_signature: partial.thought_signature,
        });
    }
    for call in made {
        emit(StreamEvent::Call(call));
    }
    emit(StreamEvent::Done(stop));
    Ok(())
}

/// A read that breaks mid-stream: a timeout is a timeout, anything else interrupted the answer.
fn stream_failure(error: std::io::Error) -> Failure {
    if error.kind() == ErrorKind::TimedOut {
        Failure::Timeout
    } else {
        Failure::Interrupted
    }
}

impl Provider for CompatProvider {
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
        if halt() {
            emit(StreamEvent::Done(StopReason::Aborted));
            return;
        }
        if let Err(failure) = self.streamed(request, halt, emit) {
            emit(StreamEvent::Failed(failure));
        }
    }
}
