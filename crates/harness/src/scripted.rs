//! A Provider that plays a script, one [`Reply`] per Step, with no network. Tests use it as the
//! Model; it also records every request so a test can check what the Model was sent.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::mpsc::Receiver;

use crate::provider::{Failure, Provider, Request, StopReason, StreamEvent};

/// One scripted Step: the deltas it streams and how it ends.
pub struct Reply {
    gate: Option<Receiver<()>>,
    chunks: Vec<String>,
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
            end,
        }
    }
}

/// A Provider whose Steps are played from a queue. A Step with nothing left to play fails as
/// unexpected, so a test that asks more than it scripted fails loudly.
pub struct ScriptedProvider {
    name: String,
    model: String,
    replies: Mutex<VecDeque<Reply>>,
    requests: Mutex<Vec<Request>>,
}

impl ScriptedProvider {
    /// A Provider called `name` sending `model`, playing `replies` in order.
    pub fn new(name: &str, model: &str, replies: Vec<Reply>) -> Self {
        Self {
            name: name.to_owned(),
            model: model.to_owned(),
            replies: Mutex::new(replies.into()),
            requests: Mutex::new(Vec::new()),
        }
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
        match reply.end {
            Some(Ok(stop)) => emit(StreamEvent::Done(stop)),
            Some(Err(failure)) => emit(StreamEvent::Failed(failure)),
            None => {}
        }
    }
}
