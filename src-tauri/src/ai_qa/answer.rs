//! One question's Run in the harness: the steps `ai_run_start` takes, on the calling thread, with the
//! answer read back from the saved entries.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mnemora_ai::{
    Ask, Choice, EntryPayload, Handles, LocalContext, RunEvent, RunOutcome, Steering, Toolset,
};
use mnemora_core::db::Connection;
use mnemora_core::util::new_id;
use mnemora_harness::{Control, Provider, Request};
use serde_json::Value;

use super::knobs::BEFORE_ONLY_TOOLS;
use crate::state::lock;

/// One Tool call of a Run, as the report shows it: its arguments and the size of its result, with
/// the row count for the change Tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallLine {
    /// The Tool's name.
    pub name: String,
    /// The arguments the Model sent (JSON text).
    pub arguments: String,
    /// The result's size in bytes, observation label included.
    pub result_bytes: usize,
    /// Whether the Tool refused the call.
    pub is_error: bool,
    /// The rows a change Tool returned across its days; none for other Tools or a refusal.
    pub rows: Option<usize>,
}

/// One question's Run: its Steps, Tool calls, final answer, the app's note and how long it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The question as asked.
    pub question: String,
    /// Model requests (Steps) the Run made.
    pub steps: usize,
    /// The Tool calls in order.
    pub calls: Vec<CallLine>,
    /// The final answer's Markdown; empty when the Run gave none.
    pub text: String,
    /// What the app would tell the person beside the answer, if anything.
    pub app_note: Option<String>,
    /// Wall-clock time of the Run.
    pub elapsed: Duration,
    /// The last request the Provider was sent in this Run, for the dump. None when it sent none.
    pub last_request: Option<Request>,
}

impl Answer {
    /// A question that could not be run at all: no Steps, and the reason as the app note.
    pub fn failed(question: &str, note: &str) -> Self {
        Self {
            question: question.to_owned(),
            steps: 0,
            calls: Vec::new(),
            text: String::new(),
            app_note: Some(note.to_owned()),
            elapsed: Duration::ZERO,
            last_request: None,
        }
    }
}

/// Asks one question as a new Conversation, through the same steps `ai_run_start` takes, and reads
/// the answer back from the saved entries. The Run is on this thread; `db` is locked only while a
/// row is read or written, as in the app.
pub fn answer(
    db: &Mutex<Connection>,
    provider: Arc<dyn Provider>,
    choice: &Choice,
    question: &str,
    local: &LocalContext,
    toolset: Toolset,
) -> Answer {
    let ask = Ask {
        conversation_id: new_id(),
        continues: false,
        text: question.to_owned(),
        local: local.clone(),
        choice: choice.clone(),
        branch_of: None,
    };
    let started = Instant::now();
    let question_id = match mnemora_ai::begin(db, &ask) {
        Ok(id) => id,
        Err(error) => return Answer::failed(question, &error.message),
    };
    let control = Control::new();
    let steering = Steering::new();
    let mut events = Vec::new();
    mnemora_ai::execute(
        db,
        provider,
        &ask,
        &question_id,
        toolset,
        Handles {
            control: &control,
            steering: &steering,
        },
        &mut |event| events.push(event),
    );
    let elapsed = started.elapsed();
    let steps = events
        .iter()
        .filter(|event| matches!(event, RunEvent::StepStarted))
        .count();
    let outcome = events.iter().rev().find_map(|event| match event {
        RunEvent::RunEnded { outcome } => Some(outcome.clone()),
        _ => None,
    });
    let view = match mnemora_ai::conversation(&lock(db), &ask.conversation_id, false) {
        Ok(view) => view,
        Err(error) => return Answer::failed(question, &error.message),
    };
    let mut calls = Vec::new();
    let mut text = String::new();
    let mut budget = None;
    for entry in &view.entries {
        match &entry.payload {
            EntryPayload::ToolResult {
                name,
                arguments,
                result,
                is_error,
                ..
            } => calls.push(CallLine {
                name: name.clone(),
                arguments: arguments.clone(),
                result_bytes: result.len(),
                is_error: *is_error,
                rows: rows_of(name, result),
            }),
            EntryPayload::Assistant {
                text: reply,
                budget_note,
                ..
            } => {
                text.clone_from(reply);
                budget.clone_from(budget_note);
            }
            _ => {}
        }
    }
    let app_note = match outcome {
        Some(RunOutcome::Failed { message, .. }) => Some(message),
        Some(RunOutcome::Stopped) => Some("The Run was stopped.".to_owned()),
        _ => budget,
    };
    Answer {
        question: question.to_owned(),
        steps,
        calls,
        text,
        app_note,
        elapsed,
        last_request: None,
    }
}

/// The rows a change Tool returned across its days, from the JSON after the observation line.
fn rows_of(name: &str, result: &str) -> Option<usize> {
    if !BEFORE_ONLY_TOOLS.contains(&name) {
        return None;
    }
    let (_, body) = result.split_once('\n')?;
    let value: Value = serde_json::from_str(body).ok()?;
    let days = value.get("days")?.as_array()?;
    Some(
        days.iter()
            .filter_map(|day| day.get("rows")?.as_array().map(Vec::len))
            .sum(),
    )
}
