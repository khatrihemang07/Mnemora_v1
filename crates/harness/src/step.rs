//! One Step of the loop: ask the Provider once and collect the reply.

use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

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

/// How often a waiting Step checks whether it must stop.
const POLL: Duration = Duration::from_millis(25);

/// Runs one Step. `on_text` sees each delta as it arrives. A stream that ends without `Done` is a
/// bad response, so a Provider that stops early cannot pass for an answer.
///
/// The Provider runs on its own thread, so a blocking reply (one HTTP call that takes up to its
/// whole timeout) cannot hold the Run: once `halt` returns true the Step ends as `Aborted` with
/// what it has, and whatever the Provider sends afterwards is dropped with its channel.
pub fn run_step(
    provider: Arc<dyn Provider>,
    request: Request,
    halt: Arc<dyn Fn() -> bool + Send + Sync>,
    on_text: &mut dyn FnMut(&str),
) -> Result<Answer, Failure> {
    let (events, received) = mpsc::channel::<StreamEvent>();
    let provider_halt = Arc::clone(&halt);
    // Detached on purpose: the thread ends when its reply does, and nothing joins it.
    std::thread::spawn(move || {
        provider.stream(&request, &*provider_halt, &mut |event| {
            let _ = events.send(event);
        });
    });
    let mut collected = Collected::default();
    loop {
        match received.recv_timeout(POLL) {
            Ok(StreamEvent::TextDelta(delta)) => {
                on_text(&delta);
                collected.text.push_str(&delta);
            }
            Ok(StreamEvent::Call(call)) => collected.calls.push(call),
            Ok(StreamEvent::Sources(ids)) => collected.sources = ids,
            Ok(StreamEvent::Correction(message)) => collected.correction = Some(message),
            Ok(StreamEvent::Done(stop)) => return Ok(collected.answer(stop)),
            Ok(StreamEvent::Failed(failure)) => return Err(failure),
            Err(RecvTimeoutError::Timeout) if halt() => {
                return Ok(collected.answer(StopReason::Aborted));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err(Failure::BadResponse),
        }
    }
}

/// What a Step has received so far.
#[derive(Default)]
struct Collected {
    text: String,
    calls: Vec<Call>,
    sources: Vec<String>,
    correction: Option<String>,
}

impl Collected {
    fn answer(self, stop: StopReason) -> Answer {
        Answer {
            text: self.text,
            stop,
            calls: self.calls,
            sources: self.sources,
            correction: self.correction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripted::{Reply, ScriptedProvider};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::time::{Duration, Instant};

    /// A Provider whose reply waits on a gate and ignores `halt`, as a blocking HTTP call does.
    struct Blocking {
        gate: Mutex<Receiver<()>>,
    }

    impl Provider for Blocking {
        fn name(&self) -> &str {
            "Blocking"
        }

        fn model(&self) -> &str {
            "m"
        }

        fn stream(
            &self,
            _request: &Request,
            _halt: &dyn Fn() -> bool,
            emit: &mut dyn FnMut(StreamEvent),
        ) {
            let _ = self.gate.lock().unwrap().recv();
            emit(StreamEvent::TextDelta("late".into()));
            emit(StreamEvent::Done(StopReason::Stop));
        }
    }

    fn blocking() -> (Sender<()>, Arc<dyn Provider>) {
        let (release, gate) = channel();
        (
            release,
            Arc::new(Blocking {
                gate: Mutex::new(gate),
            }),
        )
    }

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
        let answer = run_step(
            Arc::new(provider),
            request(),
            Arc::new(|| false),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(answer.correction.as_deref(), Some("Send one object."));
        assert!(answer.sources.is_empty());
    }

    #[test]
    fn collects_the_deltas_in_order_into_the_answer() {
        let provider = ScriptedProvider::new("Scripted", "m", vec![Reply::text(&["Hel", "lo"])]);
        let mut seen = Vec::new();
        let answer = run_step(
            Arc::new(provider),
            request(),
            Arc::new(|| false),
            &mut |d| {
                seen.push(d.to_owned());
            },
        )
        .unwrap();
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
        let result = run_step(
            Arc::new(provider),
            request(),
            Arc::new(|| false),
            &mut |_| {},
        );
        assert_eq!(result, Err(Failure::Timeout));
    }

    #[test]
    fn a_halt_ends_a_step_whose_provider_is_blocked_at_once_and_its_late_reply_is_ignored() {
        let (release, provider) = blocking();
        let halted = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&halted);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            flag.store(true, Ordering::SeqCst);
        });
        let started = Instant::now();
        let mut seen = Vec::new();
        let halt_flag = Arc::clone(&halted);
        let answer = run_step(
            provider,
            request(),
            Arc::new(move || halt_flag.load(Ordering::SeqCst)),
            &mut |d| seen.push(d.to_owned()),
        )
        .unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the Step ends at once"
        );
        assert_eq!(answer.stop, StopReason::Aborted);
        assert!(answer.text.is_empty());
        assert!(
            seen.is_empty(),
            "nothing the late reply says reaches the caller"
        );
        // The blocked provider answers now; its reply goes nowhere.
        let _ = release.send(());
    }

    #[test]
    fn a_stream_that_ends_without_done_is_a_bad_response() {
        let provider =
            ScriptedProvider::new("Scripted", "m", vec![Reply::text_without_end(&["partial"])]);
        let result = run_step(
            Arc::new(provider),
            request(),
            Arc::new(|| false),
            &mut |_| {},
        );
        assert_eq!(result, Err(Failure::BadResponse));
    }
}
