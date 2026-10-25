//! AI commands. Thin: the rules live in `mnemora-ai`. A Run answers on its own thread and reports
//! through the typed `AiRunEvent`, so it goes on when the person leaves the page.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use mnemora_ai::{Ask, ConversationView, LocalContext, NoProvider};
use mnemora_core::error::{AppError, ErrorCode};
use mnemora_core::util::new_id;
use mnemora_harness::Provider;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::AiRunEvent;
use crate::state::{AppState, lock};

/// Which Provider answers the next Run, and which Conversations have a Run answering now.
pub struct AiRuntime {
    provider: Mutex<Arc<dyn Provider>>,
    running: Arc<Mutex<HashSet<String>>>,
}

impl Default for AiRuntime {
    fn default() -> Self {
        Self {
            provider: Mutex::new(Arc::new(NoProvider)),
            running: Arc::default(),
        }
    }
}

impl AiRuntime {
    /// Replaces the Provider that answers from now on (tests install scripted ones).
    #[cfg(test)]
    pub fn set_provider(&self, provider: Arc<dyn Provider>) {
        *lock(&self.provider) = provider;
    }

    fn provider(&self) -> Arc<dyn Provider> {
        Arc::clone(&lock(&self.provider))
    }

    /// Whether a Run is answering in `conversation` now.
    pub fn is_running(&self, conversation: &str) -> bool {
        lock(&self.running).contains(conversation)
    }

    /// Claims the Conversation for one Run; `None` while another Run holds it.
    fn reserve(&self, conversation: &str) -> Option<RunSlot> {
        if !lock(&self.running).insert(conversation.to_owned()) {
            return None;
        }
        Some(RunSlot {
            running: Arc::clone(&self.running),
            conversation: conversation.to_owned(),
        })
    }
}

/// Holds a Conversation's claim until it is dropped.
struct RunSlot {
    running: Arc<Mutex<HashSet<String>>>,
    conversation: String,
}

impl Drop for RunSlot {
    fn drop(&mut self) {
        lock(&self.running).remove(&self.conversation);
    }
}

/// A question, and the Conversation it goes into (none starts a new one).
#[derive(Debug, Clone, Deserialize, Type)]
pub struct RunRequest {
    /// The Conversation to continue, or none to start one.
    pub conversation_id: Option<String>,
    /// The question as typed.
    pub text: String,
    /// The person's calendar, for the Model.
    pub local: LocalContext,
}

/// The Run that started and the Conversation it answers in.
#[derive(Debug, Clone, Serialize, Type)]
pub struct RunStarted {
    /// The Run's id: every event of this Run carries it.
    pub run_id: String,
    /// The Conversation the question went into.
    pub conversation_id: String,
}

/// Asks a question. Returns at once; the answer arrives as `AiRunEvent`s, ending with `RunEnded`.
#[tauri::command]
#[specta::specta]
pub fn ai_run_start<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    request: RunRequest,
) -> Result<RunStarted, AppError> {
    mnemora_ai::validate_question(&request.text)?;
    mnemora_ai::validate_local(&request.local)?;
    let continues = request.conversation_id.is_some();
    let conversation_id = request.conversation_id.unwrap_or_else(new_id);
    // Claimed before the check: a Run's first write happens on its thread, so a Conversation that
    // is answering may not be on disk yet.
    let slot = state.ai.reserve(&conversation_id).ok_or_else(|| {
        AppError::new(
            ErrorCode::Validation,
            "This Conversation is still answering.",
        )
    })?;
    if continues {
        mnemora_ai::ensure_conversation(&lock(&state.db), &conversation_id)?;
    }
    let run_id = new_id();
    let started = RunStarted {
        run_id: run_id.clone(),
        conversation_id: conversation_id.clone(),
    };
    let ask = Ask {
        conversation_id,
        continues,
        text: request.text,
        local: request.local,
    };
    // The question is saved before the command returns, so the Conversation can be read at once.
    let question_id = mnemora_ai::begin(&state.db, &ask)?;
    let provider = state.ai.provider();
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut slot = Some(slot);
        let state = handle.state::<AppState>();
        mnemora_ai::execute(
            &state.db,
            provider.as_ref(),
            &ask,
            &question_id,
            &mut |event| {
                // Released before the end is announced, so a read that follows it finds the
                // Conversation idle.
                if matches!(event, mnemora_ai::RunEvent::RunEnded { .. }) {
                    slot.take();
                }
                let announced = AiRunEvent {
                    run_id: run_id.clone(),
                    conversation_id: ask.conversation_id.clone(),
                    event,
                };
                let _ = tauri_specta::Event::emit(&announced, &handle);
            },
        );
    });
    Ok(started)
}

/// A Conversation with every entry, oldest first.
#[tauri::command]
#[specta::specta]
pub fn ai_conversation_get(
    state: State<'_, AppState>,
    id: String,
) -> Result<ConversationView, AppError> {
    let running = state.ai.is_running(&id);
    mnemora_ai::conversation(&lock(&state.db), &id, running)
}
