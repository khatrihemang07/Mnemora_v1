//! The agent loop (ADR 0018, after pi's nested loop). A Step asks the Model; when the reply makes
//! Tool calls they run one by one and the next Step follows, and the Step that makes none is the
//! answer. The limits, the stop-reason gate and the correction turns are the approved ones: a
//! mistake the Model can fix is sent back to it, never shown to the person as a failure.

use std::sync::Arc;

use crate::control::Control;
use crate::message::{Call, Message};
use crate::provider::{Failure, Provider, Request, StopReason, ToolDef};
use crate::step::{Answer, run_step};

/// Model requests per Run. The last one is answer-only: it is sent no Tools.
pub const MAX_REQUESTS: usize = 12;
/// Tool calls that run from one Step; the rest are refused with an error result.
pub const MAX_CALLS_PER_STEP: usize = 8;
/// The largest request sent, in bytes. A Run that would exceed it ends as too long.
pub const MAX_PROMPT_BYTES: usize = 100_000;
/// Steps in a row whose calls all repeat an earlier call. After that the Model must answer.
pub const MAX_REPEATS: usize = 3;
/// Steps in a row whose calls were unknown or badly formed. After that the Run fails.
pub const MAX_INVALID_STEPS: usize = 3;

/// Sent with the last request: no Tools, answer what there is.
pub const FINAL_NOTE: &str = "This is your last step. Answer now from what the Tools have shown, \
    and say that the answer may be incomplete.";
/// Sent once the Model has repeated itself: stop searching and answer.
pub const CLOSE_NOTE: &str = "Stop looking things up. Answer now from what the Tools have shown, \
    and say what is missing.";
/// The result of a call the Model made in a reply that was cut off. It did not run.
pub const CUT_OFF: &str =
    "The reply was cut off before these calls ran. Send them again, one at a time.";
/// The result of a call that did not run because the Run was stopped first.
pub const NOT_RUN_STOPPED: &str = "The Run was stopped before this call ran.";
/// The result of a call that did not run because the person sent a note first.
pub const NOT_RUN_SKIPPED: &str = "A note from the person came before this call, so it did not run. Ask again if you still need it.";
/// The result of a call beyond the eighth in one Step.
pub const TOO_MANY: &str =
    "Too many calls at once. Send at most eight per step; send the rest next time.";
/// The result of a call made when no Tools are offered.
pub const NO_TOOLS_NOW: &str = "No Tools are available for this answer. Answer from what you have.";

/// An item a Tool returned that an answer may cite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cited {
    /// The Entry's or Task's id.
    pub id: String,
    /// The Module it belongs to.
    pub module: String,
    /// A label the person recognises it by.
    pub title: String,
    /// Its date, when it has one.
    pub date: Option<String>,
}

/// What one Tool call produced, as the loop and the app need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// What the Model reads back.
    pub text: String,
    /// One plain line for the activity display.
    pub summary: String,
    /// Whether the call was refused.
    pub is_error: bool,
    /// Whether the call was unknown or badly formed (counts toward `MAX_INVALID_STEPS`).
    pub invalid: bool,
    /// The observation id this result is known by ("e1", "e2"...), so a repeat can name it.
    pub observation: String,
    /// The items the result carries.
    pub cited: Vec<Cited>,
}

impl Reply {
    /// A refusal the Model reads, with no items.
    pub fn refused(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            summary: "Could not read that".to_owned(),
            is_error: true,
            invalid: false,
            observation: String::new(),
            cited: Vec::new(),
        }
    }
}

/// Where the loop gets its Tools and checks answers. The AI Module implements it over the data.
pub trait ToolHost {
    /// The Tools offered this Run. Empty when the Provider cannot make Tool calls.
    fn specs(&self) -> Vec<ToolDef>;

    /// Runs one call. Never fails: a refusal is a `Reply` with `is_error` set.
    fn call(&mut self, name: &str, arguments: &str) -> Reply;

    /// Checks an answer's citations and its listed sources. An `Err` is the correction turn the
    /// Model is sent.
    fn check_answer(&self, text: &str, sources: &[String]) -> Result<(), String>;
}

/// One Step that ended, with what the Model wrote and asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRecord {
    /// The text the Model wrote.
    pub text: String,
    /// The Tool calls it made (none for an answer).
    pub calls: Vec<Call>,
    /// How the reply ended.
    pub stop: StopReason,
    /// Whether this Step was the last one, so its answer carries the budget note.
    pub budget_forced: bool,
    /// The sources the answer lists (envelope Providers); empty otherwise.
    pub sources: Vec<String>,
}

/// What the loop reports, in order, as it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A Model request is sent.
    StepStarted,
    /// A piece of the Model's text.
    TextDelta(String),
    /// A Step ended; its text and calls are kept.
    StepEnded(StepRecord),
    /// A Tool call is about to run.
    ToolStarted {
        /// The call's id.
        call_id: String,
        /// The Tool's name.
        name: String,
    },
    /// A Tool call finished (or was refused without running).
    ToolEnded {
        /// The call's id.
        call_id: String,
        /// The Tool's name.
        name: String,
        /// The arguments as the Model sent them.
        arguments: String,
        /// What it produced.
        reply: Reply,
    },
    /// A correction turn was sent to the Model.
    Correction(String),
    /// A note the person sent during the Run is now part of the branch, as a question.
    Steered(String),
}

/// How the loop ended. The final answer's `StepEnded` has been reported when it is `Answered`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The Model gave its answer.
    Answered,
    /// The person stopped the Run. What was written before is kept.
    Stopped,
    /// The Run ended without an answer.
    Failed(Failure),
}

/// Where a Step's answer went once it asked for no Tools.
enum Finished {
    /// The answer is the Run's answer.
    Answered,
    /// The answer could not be accepted and a correction was sent.
    Corrected,
    /// The answer was kept as a Step, and the person's notes were sent to the Model next.
    Steered,
    /// The Run cannot go on.
    Failed(Failure),
}

/// Runs the loop from `messages` (the branch so far, ending with the question) until the Model
/// answers, the person stops the Run, or the Run fails. `emit` sees every event in order.
/// `steer` hands over the next note the person queued, if any; notes are sent to the Model after
/// the Step's Tools finish, or after a Step that asked for none but is not the answer.
pub fn run(
    provider: Arc<dyn Provider>,
    system: &str,
    mut messages: Vec<Message>,
    host: &mut dyn ToolHost,
    control: &Control,
    steer: &mut dyn FnMut() -> Option<String>,
    emit: &mut dyn FnMut(Event),
) -> Outcome {
    let mut requests = 0;
    let mut ran: Vec<(String, String, String)> = Vec::new();
    let mut repeat_streak = 0;
    let mut invalid_streak = 0;
    let mut closing = false;
    loop {
        if control.ended() {
            return Outcome::Stopped;
        }
        let last = requests + 1 == MAX_REQUESTS;
        let withheld = last || closing;
        let request = Request {
            system: match (last, closing) {
                (true, _) => format!("{system}\n{FINAL_NOTE}"),
                (false, true) => format!("{system}\n{CLOSE_NOTE}"),
                (false, false) => system.to_owned(),
            },
            messages: messages.clone(),
            model: provider.model().to_owned(),
            tools: if withheld { Vec::new() } else { host.specs() },
        };
        if request_bytes(&request) > MAX_PROMPT_BYTES {
            return Outcome::Failed(Failure::TooLong);
        }
        requests += 1;
        control.begin_step();
        emit(Event::StepStarted);
        let halt = control.clone();
        let answer = match run_step(
            Arc::clone(&provider),
            request,
            Arc::new(move || halt.halts_step()),
            &mut |text| emit(Event::TextDelta(text.to_owned())),
        ) {
            Ok(answer) => answer,
            Err(_) if control.ended() => return Outcome::Stopped,
            Err(failure) => return Outcome::Failed(failure),
        };

        // A Step the person stopped or skipped: what it wrote is kept, and its calls do not run.
        if answer.stop == StopReason::Aborted || control.ended() {
            // On the answer-only request there is no request left for a note, so the Run ends too.
            if keep_halted(answer, host, &mut messages, &mut ran, control, emit) || last {
                return Outcome::Stopped;
            }
            deliver(steer, &mut messages, emit);
            continue;
        }

        // A reply the protocol could not read: the Model is told why and answers again. It is not
        // kept in the branch, and it fails the Run only when it keeps happening.
        if let Some(correction) = answer.correction {
            invalid_streak += 1;
            if last || invalid_streak >= MAX_INVALID_STEPS {
                return Outcome::Failed(Failure::BadResponse);
            }
            emit(Event::Correction(correction.clone()));
            messages.push(Message::Correction(correction));
            continue;
        }

        if answer.calls.is_empty() || (last && !answer.text.is_empty()) {
            match finish(answer, last, host, &mut messages, steer, emit) {
                Finished::Answered => return Outcome::Answered,
                Finished::Failed(failure) => return Outcome::Failed(failure),
                Finished::Steered => continue,
                // The answer cited or listed something this Run did not return: a correction was sent.
                Finished::Corrected => {
                    invalid_streak += 1;
                    if invalid_streak >= MAX_INVALID_STEPS {
                        return Outcome::Failed(Failure::BadResponse);
                    }
                    continue;
                }
            }
        }

        // The answer-only request asked for Tools and wrote no answer: there is no request left.
        if last {
            return Outcome::Failed(Failure::BadResponse);
        }
        let calls_made = answer.calls.len();
        let tally = run_step_calls(
            answer,
            withheld,
            host,
            &mut ran,
            &mut messages,
            control,
            emit,
        );
        if !control.ended() {
            deliver(steer, &mut messages, emit);
        }

        repeat_streak = if tally.repeats == calls_made {
            repeat_streak + 1
        } else {
            0
        };
        closing = closing || repeat_streak >= MAX_REPEATS;
        invalid_streak = if tally.invalid { invalid_streak + 1 } else { 0 };
        if invalid_streak >= MAX_INVALID_STEPS {
            return Outcome::Failed(Failure::BadResponse);
        }
    }
}

/// Keeps a Step the person stopped or skipped: what it wrote is saved, and each of its calls is
/// answered without running. Returns whether the Run ended, which a stop does and a skip does not.
fn keep_halted(
    answer: Answer,
    host: &mut dyn ToolHost,
    messages: &mut Vec<Message>,
    ran: &mut Vec<(String, String, String)>,
    control: &Control,
    emit: &mut dyn FnMut(Event),
) -> bool {
    let ended = control.ended();
    if ended || !answer.text.is_empty() || !answer.calls.is_empty() {
        // A stopped Step is saved even when empty, so the thread can say it was stopped. A skipped
        // one (Send now) was not stopped by the person: it is saved as an ordinary finished Step.
        emit(Event::StepEnded(StepRecord {
            text: answer.text.clone(),
            calls: answer.calls.clone(),
            stop: if ended {
                StopReason::Aborted
            } else {
                StopReason::Stop
            },
            budget_forced: false,
            sources: Vec::new(),
        }));
        messages.push(Message::Assistant {
            text: answer.text,
            calls: answer.calls.clone(),
        });
        let reason = if ended {
            NOT_RUN_STOPPED
        } else {
            NOT_RUN_SKIPPED
        };
        run_calls(
            &answer.calls,
            Some(reason),
            host,
            ran,
            messages,
            control,
            emit,
        );
    }
    ended
}

/// Saves a Step that asked for Tools and runs its calls. Returns what the calls came to.
fn run_step_calls(
    answer: Answer,
    withheld: bool,
    host: &mut dyn ToolHost,
    ran: &mut Vec<(String, String, String)>,
    messages: &mut Vec<Message>,
    control: &Control,
    emit: &mut dyn FnMut(Event),
) -> Tally {
    // Calls that do not run: the reply was cut off, or no Tools are offered now.
    let not_run = if answer.stop == StopReason::Length {
        Some(CUT_OFF)
    } else if withheld {
        Some(NO_TOOLS_NOW)
    } else {
        None
    };
    emit(Event::StepEnded(StepRecord {
        text: answer.text.clone(),
        calls: answer.calls.clone(),
        stop: answer.stop,
        budget_forced: false,
        sources: Vec::new(),
    }));
    messages.push(Message::Assistant {
        text: answer.text,
        calls: answer.calls.clone(),
    });
    run_calls(&answer.calls, not_run, host, ran, messages, control, emit)
}

/// Sends every queued note to the Model, in order, as questions of the branch.
fn deliver(
    steer: &mut dyn FnMut() -> Option<String>,
    messages: &mut Vec<Message>,
    emit: &mut dyn FnMut(Event),
) {
    while let Some(note) = steer() {
        messages.push(Message::User(note.clone()));
        emit(Event::Steered(note));
    }
}

/// Ends the Run on a Step that asked for no Tools. A note queued before the answer is saved turns
/// the Step into one of the branch, and the Model is asked again with the note.
fn finish(
    answer: Answer,
    last: bool,
    host: &dyn ToolHost,
    messages: &mut Vec<Message>,
    steer: &mut dyn FnMut() -> Option<String>,
    emit: &mut dyn FnMut(Event),
) -> Finished {
    if answer.text.trim().is_empty() {
        return Finished::Failed(Failure::BadResponse);
    }
    if let Err(correction) = host.check_answer(&answer.text, &answer.sources) {
        if last {
            return Finished::Failed(Failure::BadResponse);
        }
        emit(Event::StepEnded(StepRecord {
            text: answer.text.clone(),
            calls: Vec::new(),
            stop: answer.stop,
            budget_forced: false,
            sources: answer.sources.clone(),
        }));
        messages.push(Message::Assistant {
            text: answer.text,
            calls: Vec::new(),
        });
        emit(Event::Correction(correction.clone()));
        messages.push(Message::Correction(correction));
        return Finished::Corrected;
    }
    if !last && let Some(note) = steer() {
        emit(Event::StepEnded(StepRecord {
            text: answer.text.clone(),
            calls: Vec::new(),
            stop: answer.stop,
            budget_forced: false,
            sources: answer.sources.clone(),
        }));
        messages.push(Message::Assistant {
            text: answer.text,
            calls: Vec::new(),
        });
        messages.push(Message::User(note.clone()));
        emit(Event::Steered(note));
        deliver(steer, messages, emit);
        return Finished::Steered;
    }
    emit(Event::StepEnded(StepRecord {
        text: answer.text,
        calls: Vec::new(),
        stop: answer.stop,
        budget_forced: last,
        sources: answer.sources,
    }));
    Finished::Answered
}

/// What one Step's calls came to: whether any was unknown or badly formed, and how many repeated
/// an earlier call.
struct Tally {
    invalid: bool,
    repeats: usize,
}

/// Runs a Step's calls one by one, in order. A call that cannot run is answered with its reason,
/// and once the Run is stopped or the Step skipped, the rest are answered without running.
fn run_calls(
    calls: &[Call],
    not_run: Option<&str>,
    host: &mut dyn ToolHost,
    ran: &mut Vec<(String, String, String)>,
    messages: &mut Vec<Message>,
    control: &Control,
    emit: &mut dyn FnMut(Event),
) -> Tally {
    let mut tally = Tally {
        invalid: false,
        repeats: 0,
    };
    for (index, call) in calls.iter().enumerate() {
        emit(Event::ToolStarted {
            call_id: call.id.clone(),
            name: call.name.clone(),
        });
        let blocked = not_run.or_else(|| {
            if control.ended() {
                Some(NOT_RUN_STOPPED)
            } else if control.skipped() {
                Some(NOT_RUN_SKIPPED)
            } else {
                None
            }
        });
        let reply = if let Some(text) = blocked {
            Reply::refused(text)
        } else if index >= MAX_CALLS_PER_STEP {
            Reply::refused(TOO_MANY)
        } else if let Some((_, _, observation)) = ran
            .iter()
            .find(|(name, arguments, _)| *name == call.name && *arguments == call.arguments)
        {
            tally.repeats += 1;
            Reply::refused(&format!(
                "This repeats call {observation}, whose result is above. Use that result."
            ))
        } else {
            let reply = host.call(&call.name, &call.arguments);
            if reply.invalid {
                tally.invalid = true;
            } else {
                ran.push((
                    call.name.clone(),
                    call.arguments.clone(),
                    reply.observation.clone(),
                ));
            }
            reply
        };
        messages.push(Message::Tool {
            call_id: call.id.clone(),
            name: call.name.clone(),
            text: reply.text.clone(),
            is_error: reply.is_error,
        });
        emit(Event::ToolEnded {
            call_id: call.id.clone(),
            name: call.name.clone(),
            arguments: call.arguments.clone(),
            reply,
        });
    }
    tally
}

/// The bytes a request sends: the prompt, the branch and the Tools' schemas.
fn request_bytes(request: &Request) -> usize {
    let branch: usize = request
        .messages
        .iter()
        .map(|message| match message {
            Message::User(text) | Message::Correction(text) | Message::Tool { text, .. } => {
                text.len()
            }
            Message::Assistant { text, calls } => {
                text.len()
                    + calls
                        .iter()
                        .map(|c| c.arguments.len() + c.name.len())
                        .sum::<usize>()
            }
        })
        .sum();
    let tools: usize = request
        .tools
        .iter()
        .map(|tool| tool.name.len() + tool.about.len() + tool.schema.len())
        .sum();
    request.system.len() + branch + tools
}
