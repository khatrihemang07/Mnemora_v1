//! One Run: the question is saved, one Step asks the Provider, the answer is saved, and every
//! step is reported through `emit` in order. Runs never throw: a failure is the last event.

use std::sync::{Mutex, MutexGuard, PoisonError};

use mnemora_core::db::{Connection, physical_now};
use mnemora_core::error::AppResult;
use mnemora_core::util::new_id;
use mnemora_harness::{Failure, Provider, Request, StopReason, run_step};

use crate::prompt;
use crate::store::{self, NewRow};
use crate::title::title_of;
use crate::types::{EntryKind, EntryPayload, LocalContext, RunEvent, RunOutcome};

/// What the person asked, and where.
#[derive(Debug, Clone)]
pub struct Ask {
    /// The Conversation the question goes into (a new id when `continues` is false).
    pub conversation_id: String,
    /// Whether the Conversation already exists. If not, it is started with this question.
    pub continues: bool,
    /// The question as typed.
    pub text: String,
    /// The person's calendar, for the system prompt.
    pub local: LocalContext,
}

/// The one sentence the person reads for each failure. The Provider's own text is never used.
pub fn failure_message(failure: Failure) -> &'static str {
    match failure {
        Failure::NoProvider => "No AI service is set up on this device yet.",
        Failure::Unreachable => {
            "Could not reach the AI service. Check its address and that it is running."
        }
        Failure::Timeout => "The AI service took too long to answer.",
        Failure::KeyRejected => "The AI service rejected the key. Check it in AI settings.",
        Failure::RateLimited => "The AI service is busy or has reached a limit. Try again soon.",
        Failure::ModelUnavailable => "The chosen Model is not available from this service.",
        Failure::ModelCannotUseTools => "The chosen Model cannot use Tools. Pick another Model.",
        Failure::BadResponse => "The AI service sent a reply Mnemora could not read.",
        Failure::TooLong => "This Conversation is too long to continue.",
        Failure::Interrupted => "The answer was interrupted. Try again.",
        Failure::Offline => "You’re offline. Your question is kept.",
        Failure::Unexpected => "Something went wrong while answering. Try again.",
    }
}

/// Writes the question before its Run starts: the Conversation is started if it is new, and the
/// question follows its newest entry. Returns the question's entry id. Done in the command, so the
/// Conversation exists by the time the command returns.
pub fn begin(db: &Mutex<Connection>, ask: &Ask) -> AppResult<String> {
    save_question(db, ask)
}

/// Runs a question that `begin` has written, to its end. `db` is locked only while a row is read
/// or written, never while the Provider streams, so the rest of the app stays responsive.
pub fn execute(
    db: &Mutex<Connection>,
    provider: &dyn Provider,
    ask: &Ask,
    question_id: &str,
    emit: &mut dyn FnMut(RunEvent),
) {
    emit(RunEvent::RunStarted);
    emit(RunEvent::EntrySaved {
        entry_id: question_id.to_owned(),
        kind: EntryKind::User,
    });
    let request = match build_request(db, provider, ask, question_id) {
        Ok(request) => request,
        Err(error) => return fail_unexpected(emit, &error),
    };
    emit(RunEvent::StepStarted);
    let answer = run_step(provider, &request, &mut |text| {
        emit(RunEvent::TextDelta {
            text: text.to_owned(),
        });
    });
    match answer {
        Ok(answer) => match save_answer(db, provider, ask, question_id, answer.text, answer.stop) {
            Ok(entry_id) => {
                emit(RunEvent::EntrySaved {
                    entry_id,
                    kind: EntryKind::Assistant,
                });
                emit(RunEvent::RunEnded {
                    outcome: RunOutcome::Answered,
                });
            }
            Err(error) => fail_unexpected(emit, &error),
        },
        Err(failure) => emit(RunEvent::RunEnded {
            outcome: RunOutcome::Failed {
                kind: failure,
                message: failure_message(failure).to_owned(),
            },
        }),
    }
}

fn fail_unexpected(emit: &mut dyn FnMut(RunEvent), error: &mnemora_core::error::AppError) {
    // The error holds no Conversation text (ids and SQL only), so it may be logged.
    tracing::error!(error = %error.message, "AI Run could not store its entries");
    emit(RunEvent::RunEnded {
        outcome: RunOutcome::Failed {
            kind: Failure::Unexpected,
            message: failure_message(Failure::Unexpected).to_owned(),
        },
    });
}

fn lock(db: &Mutex<Connection>) -> MutexGuard<'_, Connection> {
    db.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Starts the Conversation if it is new, then writes the question after the newest entry.
fn save_question(db: &Mutex<Connection>, ask: &Ask) -> AppResult<String> {
    let mut conn = lock(db);
    let tx = conn.transaction()?;
    let now = physical_now(&tx);
    if !ask.continues {
        store::insert_conversation(&tx, &ask.conversation_id, &title_of(&ask.text), now)?;
    }
    let parent = store::leaf(&tx, &ask.conversation_id)?;
    let id = new_id();
    store::insert_entry(
        &tx,
        NewRow {
            id: &id,
            conversation: &ask.conversation_id,
            parent: parent.as_deref(),
            kind: EntryKind::User,
            now,
        },
        &EntryPayload::User {
            text: ask.text.clone(),
        },
    )?;
    store::touch_conversation(&tx, &ask.conversation_id, now)?;
    tx.commit()?;
    Ok(id)
}

fn build_request(
    db: &Mutex<Connection>,
    provider: &dyn Provider,
    ask: &Ask,
    question_id: &str,
) -> AppResult<Request> {
    let conn = lock(db);
    Ok(Request {
        system: prompt::system(&ask.local),
        messages: store::branch_messages(&conn, question_id)?,
        model: provider.model().to_owned(),
    })
}

fn save_answer(
    db: &Mutex<Connection>,
    provider: &dyn Provider,
    ask: &Ask,
    question_id: &str,
    text: String,
    stop: StopReason,
) -> AppResult<String> {
    let mut conn = lock(db);
    let tx = conn.transaction()?;
    let now = physical_now(&tx);
    let id = new_id();
    store::insert_entry(
        &tx,
        NewRow {
            id: &id,
            conversation: &ask.conversation_id,
            parent: Some(question_id),
            kind: EntryKind::Assistant,
            now,
        },
        &EntryPayload::Assistant {
            text,
            stop,
            provider: provider.name().to_owned(),
            model: provider.model().to_owned(),
        },
    )?;
    store::touch_conversation(&tx, &ask.conversation_id, now)?;
    tx.commit()?;
    Ok(id)
}
