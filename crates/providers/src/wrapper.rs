//! The Local AI Wrapper: an OpenAI-compatible chat endpoint that rejects native Tool calls. Its
//! Steps are written in the JSON envelope (`envelope.rs`), and its answer comes whole when it is
//! ready. Neither the reply's raw text nor the service's error text is kept: errors are mapped to a
//! `Failure` and dropped.

use std::sync::atomic::{AtomicU64, Ordering};

use mnemora_harness::{Failure, Provider, Request, StreamEvent, ToolProtocol};
use serde_json::{Value, json};

use crate::client::{Client, read_body};
use crate::envelope::{self, Parsed};
use crate::openai::{self, Endpoint};

/// A Provider for the Local AI Wrapper. The address already includes the version path.
pub struct WrapperProvider {
    name: String,
    model: String,
    address: String,
    key: Option<String>,
    client: Client,
    /// Ids for the calls this Provider reads; unique within the Run that owns it.
    next_call: AtomicU64,
}

impl WrapperProvider {
    /// A Provider with the app's client.
    pub fn new(name: &str, model: &str, address: &str, key: Option<String>) -> Self {
        Self::with_client(Client::new(), name, model, address, key)
    }

    /// As [`WrapperProvider::new`], with the client chosen by the caller (timeouts, trusted roots).
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
            next_call: AtomicU64::new(1),
        }
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

    /// One Step: the envelope is sent as the response format, and the reply is read as one.
    fn step(&self, request: &Request, emit: &mut dyn FnMut(StreamEvent)) -> Result<(), Failure> {
        let finish_only = request.tools.is_empty();
        let body = self.body(request, finish_only);
        let response = self.client.post_json(
            &format!("{}/chat/completions", self.address),
            &body,
            self.key.as_deref(),
        )?;
        let status = response.status().as_u16();
        let text = read_body(response)?;
        if !(200..300).contains(&status) {
            return Err(openai::status_failure(status, &text, Endpoint::Chat));
        }
        let (content, stop) = openai::parse_answer(&text)?;
        let parsed = envelope::parse(&content, !finish_only, || {
            format!("w{}", self.next_call.fetch_add(1, Ordering::Relaxed))
        });
        match parsed {
            Parsed::Calls(calls) => {
                for call in calls {
                    emit(StreamEvent::Call(call));
                }
            }
            Parsed::Answer { text, sources } => {
                emit(StreamEvent::TextDelta(text));
                emit(StreamEvent::Sources(sources));
            }
            Parsed::Correction(message) => emit(StreamEvent::Correction(message.to_owned())),
        }
        emit(StreamEvent::Done(stop));
        Ok(())
    }

    /// The chat body: the system prompt with the envelope's instructions, the branch as turns,
    /// and the envelope as the response format. No native `tools` are sent (the Wrapper refuses
    /// them), and no output limit.
    fn body(&self, request: &Request, finish_only: bool) -> Value {
        let instructions = envelope::instructions(&request.tools);
        let system = if request.system.is_empty() {
            instructions
        } else {
            format!("{}\n\n{instructions}", request.system)
        };
        let mut messages = vec![json!({ "role": "system", "content": system })];
        messages.extend(envelope::turns(&request.messages));
        json!({
            "model": request.model,
            "messages": messages,
            "stream": false,
            "response_format": envelope::response_format(finish_only),
        })
    }
}

impl Provider for WrapperProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn tool_protocol(&self) -> ToolProtocol {
        ToolProtocol::Envelope
    }

    fn stream(&self, request: &Request, emit: &mut dyn FnMut(StreamEvent)) {
        if let Err(failure) = self.step(request, emit) {
            emit(StreamEvent::Failed(failure));
        }
    }
}
