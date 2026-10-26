//! One Step of the loop: ask the Provider once and collect the reply.

use crate::message::Call;
use crate::provider::{Failure, Provider, Request, StopReason, StreamEvent};

/// A finished Step's reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The whole text, as streamed.
    pub text: String,
    /// How the reply ended.
    pub stop: StopReason,
    /// The Tool calls the reply made, in order.
    pub calls: Vec<Call>,
    /// The ids the answer lists as sources (envelope Providers). Empty otherwise.
    pub sources: Vec<String>,
    /// The correction the Model must be sent because its reply was not valid. The reply's text
    /// is not an answer when this is set.
    pub correction: Option<String>,
}

/// Runs one Step. `on_text` sees each delta as it arrives. A stream that ends without `Done` is a
/// bad response, so a Provider that stops early cannot pass for an answer.
pub fn run_step(
    provider: &dyn Provider,
    request: &Request,
    on_text: &mut dyn FnMut(&str),
) -> Result<Answer, Failure> {
    let mut text = String::new();
    let mut calls = Vec::new();
    let mut sources = Vec::new();
    let mut correction = None;
    let mut end: Option<Result<StopReason, Failure>> = None;
    provider.stream(request, &mut |event| match event {
        StreamEvent::TextDelta(delta) => {
            on_text(&delta);
            text.push_str(&delta);
        }
        StreamEvent::Call(call) => calls.push(call),
        StreamEvent::Sources(ids) => sources = ids,
        StreamEvent::Correction(message) => correction = Some(message),
        StreamEvent::Done(stop) => end = Some(Ok(stop)),
        StreamEvent::Failed(failure) => end = Some(Err(failure)),
    });
    match end {
        Some(Ok(stop)) => Ok(Answer {
            text,
            stop,
            calls,
            sources,
            correction,
        }),
        Some(Err(failure)) => Err(failure),
        None => Err(Failure::BadResponse),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripted::{Reply, ScriptedProvider};

    fn request() -> Request {
        Request {
            system: String::new(),
            messages: Vec::new(),
            model: "m".into(),
            tools: Vec::new(),
        }
    }

    #[test]
    fn a_correction_and_sources_are_carried_with_the_step() {
        let provider =
            ScriptedProvider::new("Scripted", "m", vec![Reply::corrected("Send one object.")]);
        let answer = run_step(&provider, &request(), &mut |_| {}).unwrap();
        assert_eq!(answer.correction.as_deref(), Some("Send one object."));
        assert!(answer.sources.is_empty());
    }

    #[test]
    fn collects_the_deltas_in_order_into_the_answer() {
        let provider = ScriptedProvider::new("Scripted", "m", vec![Reply::text(&["Hel", "lo"])]);
        let mut seen = Vec::new();
        let answer = run_step(&provider, &request(), &mut |d| seen.push(d.to_owned())).unwrap();
        assert_eq!(answer.text, "Hello");
        assert_eq!(answer.stop, StopReason::Stop);
        assert!(answer.calls.is_empty());
        assert_eq!(seen, ["Hel", "lo"]);
    }

    #[test]
    fn a_failure_is_returned_and_its_partial_text_dropped() {
        let provider = ScriptedProvider::new(
            "Scripted",
            "m",
            vec![Reply::text_then_fail(&["half"], Failure::Timeout)],
        );
        let result = run_step(&provider, &request(), &mut |_| {});
        assert_eq!(result, Err(Failure::Timeout));
    }

    #[test]
    fn a_stream_that_ends_without_done_is_a_bad_response() {
        let provider =
            ScriptedProvider::new("Scripted", "m", vec![Reply::text_without_end(&["partial"])]);
        let result = run_step(&provider, &request(), &mut |_| {});
        assert_eq!(result, Err(Failure::BadResponse));
    }
}
