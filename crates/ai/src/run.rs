//! One Run: the question is saved, one Step asks the Provider, the answer is saved, and every
//! step is reported through `emit` in order. Runs never throw: a failure is the last event.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use mnemora_core::db::{Connection, physical_now};
use mnemora_core::error::AppResult;
use mnemora_core::tool::{Calendar, Tool};
use mnemora_core::util::new_id;
use mnemora_harness::Cited;
use mnemora_harness::agent::{self, Event, MAX_REQUESTS, Outcome};
use mnemora_harness::{Failure, Message, Provider, ToolProtocol};

use crate::prompt;
use crate::provider_store;
use crate::store::{self, NewRow};
use crate::title::title_of;
use crate::tools::{Toolbox, cited_ids};
use crate::types::{
    CallRecord, EntryKind, EntryPayload, LocalContext, RunEvent, RunOutcome, Source,
};

/// The Provider and Model a question is asked with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// The Provider's id.
    pub provider_id: String,
    /// The Provider's name, kept as text in the Conversation.
    pub provider_name: String,
    /// The Model id.
    pub model: String,
}

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
    /// The Provider and Model that answer it.
    pub choice: Choice,
}

/// The one sentence the person reads for each failure. The Provider's own text is never used.
pub fn failure_message(failure: Failure) -> &'static str {
    match failure {
        Failure::NoProvider => "No AI service is set up on this device yet.",
        Failure::Unreachable => {
            "Could not reach the AI service. Check its address and that it is running."
        }
        Failure::Timeout => "The AI service took too long to answer.",
        Failure::KeyRejected => "The AI service rejected the key. Check the key and try again.",
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

/// The note added to an answer that the Run ran out of steps for.
pub fn budget_note() -> String {
    format!("Answered after {MAX_REQUESTS} steps; there may be more to find")
}

/// Runs a question that `begin` has written, to its end. `db` is locked only while a row is read
/// or written, never while the Provider streams or a Tool reads, so the rest of the app stays
/// responsive. `tools` are the Tools this Run may call and `off` names the Modules turned off.
pub fn execute(
    db: &Mutex<Connection>,
    provider: &dyn Provider,
    ask: &Ask,
    question_id: &str,
    tools: Vec<Box<dyn Tool>>,
    off: &[String],
    emit: &mut dyn FnMut(RunEvent),
) {
    emit(RunEvent::RunStarted);
    emit(RunEvent::EntrySaved {
        entry_id: question_id.to_owned(),
        kind: EntryKind::User,
    });
    let messages = match branch(db, question_id) {
        Ok(messages) => messages,
        Err(error) => return fail_unexpected(emit, &error),
    };
    let native = provider.tool_protocol() != ToolProtocol::None;
    let mut toolbox = Toolbox::new(
        db,
        tools,
        Calendar {
            today: ask.local.today.clone(),
        },
        native,
    );
    let system = prompt::system(&ask.local, &toolbox.available(), off, native);
    let mut recorder = Recorder {
        db,
        ask,
        provider,
        parent: question_id.to_owned(),
        cited: HashMap::new(),
        broken: false,
    };
    let outcome = agent::run(provider, &system, messages, &mut toolbox, &mut |event| {
        recorder.handle(event, emit);
    });
    let outcome = if recorder.broken {
        Outcome::Failed(Failure::Unexpected)
    } else {
        outcome
    };
    match outcome {
        Outcome::Answered => emit(RunEvent::RunEnded {
            outcome: RunOutcome::Answered,
        }),
        Outcome::Failed(kind) => emit(RunEvent::RunEnded {
            outcome: RunOutcome::Failed {
                kind,
                message: failure_message(kind).to_owned(),
            },
        }),
    }
}

/// Saves a Run's entries as its steps happen, each after the one before, and remembers the items
/// its Tools returned so that an answer's Sources can be found. Once an entry cannot be saved the
/// Run is `broken` and ends as unexpected.
struct Recorder<'a> {
    db: &'a Mutex<Connection>,
    ask: &'a Ask,
    provider: &'a dyn Provider,
    /// The last entry saved: what the next one follows.
    parent: String,
    /// Every item a Tool returned in this Run, by id.
    cited: HashMap<String, Source>,
    broken: bool,
}

impl Recorder<'_> {
    fn handle(&mut self, event: Event, emit: &mut dyn FnMut(RunEvent)) {
        if self.broken {
            return;
        }
        match event {
            Event::StepStarted => emit(RunEvent::StepStarted),
            Event::TextDelta(text) => emit(RunEvent::TextDelta { text }),
            Event::StepEnded(record) => {
                let sources = if record.calls.is_empty() {
                    cited_sources(&record.text, &record.sources, &self.cited)
                } else {
                    Vec::new()
                };
                let payload = EntryPayload::Assistant {
                    text: record.text,
                    stop: record.stop,
                    provider: self.provider.name().to_owned(),
                    model: self.provider.model().to_owned(),
                    calls: record
                        .calls
                        .into_iter()
                        .map(|c| CallRecord {
                            id: c.id,
                            name: c.name,
                            arguments: c.arguments,
                        })
                        .collect(),
                    sources,
                    budget_note: record.budget_forced.then(budget_note),
                };
                self.save(EntryKind::Assistant, &payload, emit);
            }
            Event::ToolStarted { call_id, name } => emit(RunEvent::ToolStarted { call_id, name }),
            Event::ToolEnded {
                call_id,
                name,
                arguments,
                reply,
            } => {
                for item in &reply.cited {
                    self.cited.insert(item.id.clone(), source_of(item));
                }
                let payload = EntryPayload::ToolResult {
                    call_id: call_id.clone(),
                    name: name.clone(),
                    arguments,
                    result: reply.text,
                    is_error: reply.is_error,
                    observation_id: reply.observation,
                    summary: reply.summary.clone(),
                };
                self.save(EntryKind::ToolResult, &payload, emit);
                emit(RunEvent::ToolEnded {
                    call_id,
                    name,
                    summary: reply.summary,
                    is_error: reply.is_error,
                });
            }
            Event::Correction(text) => {
                let payload = EntryPayload::Correction { text };
                self.save(EntryKind::Correction, &payload, emit);
            }
        }
    }

    fn save(&mut self, kind: EntryKind, payload: &EntryPayload, emit: &mut dyn FnMut(RunEvent)) {
        self.broken = !record_entry(self.db, self.ask, &mut self.parent, kind, payload, emit);
    }
}

fn source_of(item: &Cited) -> Source {
    Source {
        id: item.id.clone(),
        module: item.module.clone(),
        title: item.title.clone(),
        date: item.date.clone(),
    }
}

/// Saves one entry after `parent`, announces it, and makes it the parent of the next. Returns
/// false when it could not be saved, which ends the Run as unexpected.
fn record_entry(
    db: &Mutex<Connection>,
    ask: &Ask,
    parent: &mut String,
    kind: EntryKind,
    payload: &EntryPayload,
    emit: &mut dyn FnMut(RunEvent),
) -> bool {
    match save_entry(db, ask, parent, payload) {
        Ok(id) => {
            emit(RunEvent::EntrySaved {
                entry_id: id.clone(),
                kind,
            });
            *parent = id;
            true
        }
        Err(error) => {
            // The error holds no Conversation text (ids and SQL only), so it may be logged.
            tracing::error!(error = %error.message, "AI Run could not store its entries");
            false
        }
    }
}

/// The Sources of an answer: the items it cites that a Tool returned in this Run.
fn cited_sources(text: &str, listed: &[String], cited: &HashMap<String, Source>) -> Vec<Source> {
    let mut ids = cited_ids(text);
    for id in listed {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    ids.iter().filter_map(|id| cited.get(id).cloned()).collect()
}

/// The branch that ends at `question_id`, read under the lock.
fn branch(db: &Mutex<Connection>, question_id: &str) -> AppResult<Vec<Message>> {
    store::branch_messages(&lock(db), question_id)
}

/// Writes one entry after `parent` in the Conversation, and marks the Conversation changed.
fn save_entry(
    db: &Mutex<Connection>,
    ask: &Ask,
    parent: &str,
    payload: &EntryPayload,
) -> AppResult<String> {
    let kind = match payload {
        EntryPayload::User { .. } => EntryKind::User,
        EntryPayload::Assistant { .. } => EntryKind::Assistant,
        EntryPayload::ToolResult { .. } => EntryKind::ToolResult,
        EntryPayload::Correction { .. } => EntryKind::Correction,
        EntryPayload::ModelChange { .. } => EntryKind::ModelChange,
    };
    let mut conn = lock(db);
    let tx = conn.transaction()?;
    let now = physical_now(&tx);
    let id = new_id();
    store::insert_entry(
        &tx,
        NewRow {
            id: &id,
            conversation: &ask.conversation_id,
            parent: Some(parent),
            kind,
            now,
        },
        payload,
    )?;
    store::touch_conversation(&tx, &ask.conversation_id, now)?;
    tx.commit()?;
    Ok(id)
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
    let mut parent = store::leaf(&tx, &ask.conversation_id)?;
    let header = store::live_conversation(&tx, &ask.conversation_id)?
        .ok_or(crate::error::AiError::NotFound)?;
    let same_model = header.provider_id.as_deref() == Some(ask.choice.provider_id.as_str())
        && header.model.as_deref() == Some(ask.choice.model.as_str());
    if !same_model {
        // A Model change is an entry of its own, so the branch shows which Model answered after it.
        let change = new_id();
        store::insert_entry(
            &tx,
            NewRow {
                id: &change,
                conversation: &ask.conversation_id,
                parent: parent.as_deref(),
                kind: EntryKind::ModelChange,
                now,
            },
            &EntryPayload::ModelChange {
                provider_name: ask.choice.provider_name.clone(),
                model: ask.choice.model.clone(),
            },
        )?;
        store::set_model(
            &tx,
            &ask.conversation_id,
            &ask.choice.provider_id,
            &ask.choice.provider_name,
            &ask.choice.model,
        )?;
        parent = Some(change);
    }
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
    provider_store::record_use(&tx, &ask.choice.provider_id, &ask.choice.model, now)?;
    tx.commit()?;
    Ok(id)
}
