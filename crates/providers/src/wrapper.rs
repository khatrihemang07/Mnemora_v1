//! The Local AI Wrapper and every OpenAI-compatible endpoint: one adapter, one wire format. The
//! Wrapper answers whole (its answer appears when ready); a streaming endpoint sends text as it
//! comes. Neither keeps the reply's raw text: errors are mapped to a `Failure` and dropped.

use std::io::{BufRead, BufReader, ErrorKind};

use mnemora_harness::{Failure, Provider, Request, StopReason, StreamEvent};

use crate::client::{Client, read_body};
use crate::openai::{self, Endpoint, SseEvent};

/// A Provider for any service that speaks the chat format of `OpenAI`. The address
/// already includes the version path.
pub struct WrapperProvider {
    name: String,
    model: String,
    address: String,
    key: Option<String>,
    streams: bool,
    client: Client,
}

impl WrapperProvider {
    /// A Provider that waits for the whole answer, as the Local AI Wrapper answers.
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
            streams: false,
            client,
        }
    }

    /// Asks for the answer as a stream of text deltas instead of one reply.
    #[must_use]
    pub fn streaming(mut self) -> Self {
        self.streams = true;
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

    fn whole(&self, request: &Request, emit: &mut dyn FnMut(StreamEvent)) -> Result<(), Failure> {
        let body = openai::chat_body(request, false);
        let response = self.post_chat(&body)?;
        let status = response.status().as_u16();
        let text = read_body(response)?;
        if !(200..300).contains(&status) {
            return Err(openai::status_failure(status, &text, Endpoint::Chat));
        }
        let (answer, stop) = openai::parse_answer(&text)?;
        emit(StreamEvent::TextDelta(answer));
        emit(StreamEvent::Done(stop));
        Ok(())
    }

    fn streamed(
        &self,
        request: &Request,
        emit: &mut dyn FnMut(StreamEvent),
    ) -> Result<(), Failure> {
        let body = openai::chat_body(request, true);
        let response = self.post_chat(&body)?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            let text = read_body(response)?;
            return Err(openai::status_failure(status, &text, Endpoint::Chat));
        }
        let mut reader = BufReader::new(response.into_body().into_reader());
        let mut stop: Option<StopReason> = None;
        let mut line = String::new();
        loop {
            line.clear();
            let read = reader.read_line(&mut line).map_err(stream_failure)?;
            if read == 0 {
                break;
            }
            for event in openai::sse_events(line.trim_end())? {
                match event {
                    SseEvent::Text(text) => emit(StreamEvent::TextDelta(text)),
                    SseEvent::Finish(reason) => stop = Some(reason),
                    SseEvent::Done => return finish(stop, emit),
                }
            }
        }
        // The stream ended without `[DONE]`: an answer only counts once it has finished.
        finish(stop, emit)
    }

    fn post_chat(
        &self,
        body: &serde_json::Value,
    ) -> Result<ureq::http::Response<ureq::Body>, Failure> {
        self.client.post_json(
            &format!("{}/chat/completions", self.address),
            body,
            self.key.as_deref(),
        )
    }
}

/// Ends a stream: a finish reason is needed for the answer to count.
fn finish(stop: Option<StopReason>, emit: &mut dyn FnMut(StreamEvent)) -> Result<(), Failure> {
    match stop {
        Some(reason) => {
            emit(StreamEvent::Done(reason));
            Ok(())
        }
        None => Err(Failure::BadResponse),
    }
}

/// A read that breaks mid-stream: a timeout is a timeout, anything else interrupted the answer.
fn stream_failure(error: std::io::Error) -> Failure {
    if error.kind() == ErrorKind::TimedOut {
        Failure::Timeout
    } else {
        Failure::Interrupted
    }
}

impl Provider for WrapperProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn stream(&self, request: &Request, emit: &mut dyn FnMut(StreamEvent)) {
        let result = if self.streams {
            self.streamed(request, emit)
        } else {
            self.whole(request, emit)
        };
        if let Err(failure) = result {
            emit(StreamEvent::Failed(failure));
        }
    }
}
