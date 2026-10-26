//! A Provider that plays a script, one [`Reply`] per Step, with no network. Tests use it as the
//! Model; it also records every request so a test can check what the Model was sent.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::mpsc::Receiver;

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::message::Call;
use crate::provider::{Failure, Provider, Request, StopReason, StreamEvent, ToolProtocol};

/// Gives every scripted Tool call a distinct id, as a real Provider does.
static CALL_IDS: AtomicUsize = AtomicUsize::new(1);

/// One scripted Step: the deltas it streams, the Tool calls it makes and how it ends.
pub struct Reply {
    gate: Option<Receiver<()>>,
    chunks: Vec<String>,
    calls: Vec<Call>,
    sources: Vec<String>,
    correction: Option<String>,
    end: Option<Result<StopReason, Failure>>,
}

impl Reply {
    /// Streams `chunks`, then finishes normally.
    pub fn text(chunks: &[&str]) -> Self {
        Self::with(chunks, Some(Ok(StopReason::Stop)))
    }

    /// Streams `chunks`, then finishes because the reply hit the output limit.
    pub fn length(chunks: &[&str]) -> Self {
        Self::with(chunks, Some(Ok(StopReason::Length)))
    }

    /// Makes the Tool calls `(name, arguments)` in order, then finishes normally.
    pub fn calls(calls: &[(&str, &str)]) -> Self {
        Self::with(&[], Some(Ok(StopReason::Stop))).with_calls(calls)
    }

    /// Makes the Tool calls, then finishes because the reply hit the output limit.
    pub fn length_calls(calls: &[(&str, &str)]) -> Self {
        Self::with(&[], Some(Ok(StopReason::Length))).with_calls(calls)
    }

    fn with_calls(mut self, calls: &[(&str, &str)]) -> Self {
        self.calls = calls
            .iter()
            .map(|(name, arguments)| Call {
                id: format!("call-{}", CALL_IDS.fetch_add(1, Ordering::Relaxed)),
                name: (*name).to_owned(),
                arguments: (*arguments).to_owned(),
            })
            .collect();
        self
    }

    /// Streams `text` and lists `sources` as its sources, then finishes normally.
    pub fn answer_with_sources(text: &str, sources: &[&str]) -> Self {
        let mut reply = Self::with(&[text], Some(Ok(StopReason::Stop)));
        reply.sources = sources.iter().map(|id| (*id).to_owned()).collect();
        reply
    }

    /// Reports a reply that was not valid for the protocol, so `correction` is sent back.
    pub fn corrected(correction: &str) -> Self {
        let mut reply = Self::with(&[], Some(Ok(StopReason::Stop)));
        reply.correction = Some(correction.to_owned());
        reply
    }

    /// Streams `chunks`, then fails.
    pub fn text_then_fail(chunks: &[&str], failure: Failure) -> Self {
        Self::with(chunks, Some(Err(failure)))
    }

    /// Streams `chunks` and then ends without a `Done` or `Failed` event.
    pub fn text_without_end(chunks: &[&str]) -> Self {
        Self::with(chunks, None)
    }

    /// Fails at once.
    pub fn fail(failure: Failure) -> Self {
        Self::with(&[], Some(Err(failure)))
    }

    /// Waits until `gate` yields or is dropped before it streams. Tests hold a Run open with it.
    #[must_use]
    pub fn held(mut self, gate: Receiver<()>) -> Self {
        self.gate = Some(gate);
        self
    }

    fn with(chunks: &[&str], end: Option<Result<StopReason, Failure>>) -> Self {
        Self {
            gate: None,
            chunks: chunks.iter().map(|c| (*c).to_owned()).collect(),
            calls: Vec::new(),
            sources: Vec::new(),
            correction: None,
            end,
        }
    }
}

/// A Provider whose Steps are played from a queue. A Step with nothing left to play fails as
/// unexpected, so a test that asks more than it scripted fails loudly.
pub struct ScriptedProvider {
    name: String,
    model: String,
    protocol: ToolProtocol,
    replies: Mutex<VecDeque<Reply>>,
    requests: Mutex<Vec<Request>>,
}

impl ScriptedProvider {
    /// A Provider called `name` sending `model`, playing `replies` in order.
    pub fn new(name: &str, model: &str, replies: Vec<Reply>) -> Self {
        Self {
            name: name.to_owned(),
            model: model.to_owned(),
            protocol: ToolProtocol::None,
            replies: Mutex::new(replies.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Makes this Provider accept Tools, as an adapter with native Tool calls does.
    #[must_use]
    pub fn with_native_tools(mut self) -> Self {
        self.protocol = ToolProtocol::Native;
        self
    }

    /// Makes this Provider take Tools through `protocol`.
    #[must_use]
    pub fn with_tool_protocol(mut self, protocol: ToolProtocol) -> Self {
        self.protocol = protocol;
        self
    }

    /// Every request received so far, oldest first.
    pub fn requests(&self) -> Vec<Request> {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl Provider for ScriptedProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn tool_protocol(&self) -> ToolProtocol {
        self.protocol
    }

    fn stream(&self, request: &Request, emit: &mut dyn FnMut(StreamEvent)) {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request.clone());
        let next = self
            .replies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front();
        let Some(reply) = next else {
            emit(StreamEvent::Failed(Failure::Unexpected));
            return;
        };
        if let Some(gate) = &reply.gate {
            // Opened by a send or by the sender being dropped; either way the Step goes on.
            let _ = gate.recv();
        }
        for chunk in reply.chunks {
            emit(StreamEvent::TextDelta(chunk));
        }
        for call in reply.calls {
            emit(StreamEvent::Call(call));
        }
        if !reply.sources.is_empty() {
            emit(StreamEvent::Sources(reply.sources));
        }
        if let Some(correction) = reply.correction {
            emit(StreamEvent::Correction(correction));
        }
        match reply.end {
            Some(Ok(stop)) => emit(StreamEvent::Done(stop)),
            Some(Err(failure)) => emit(StreamEvent::Failed(failure)),
            None => {}
        }
    }
}
