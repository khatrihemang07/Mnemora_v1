//! AI commands. Thin: the rules live in `mnemora-ai`. A Run answers on its own thread and reports
//! through the typed `AiRunEvent`, so it goes on when the person leaves the page. Asking a Provider
//! (a connection test, a Model search) runs off the command thread and without the database lock.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mnemora_ai::{
    ConnectionTest, ConversationPage, ConversationView, KeyStore, LocalContext, ModelList,
    ModelUsage, NewProvider, Prepared, ProviderUpdate, ProviderView, SavedQuestion,
};
use mnemora_core::db::physical_now;
use mnemora_core::error::{AppError, ErrorCode};
use mnemora_core::tool::{Tool, is_enabled};
use mnemora_core::util::new_id;
use mnemora_harness::Provider;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::AiRunEvent;
use crate::state::{AppState, lock};

/// The Provider that answers the next Run, and which Conversations have a Run answering now.
pub struct AiRuntime {
    keys: KeyStore,
    /// Replaces the adapter a Run would build (tests install a scripted one).
    #[cfg_attr(not(test), allow(dead_code))]
    provider: Mutex<Option<Arc<dyn Provider>>>,
    running: Arc<Mutex<HashSet<String>>>,
}

impl AiRuntime {
    /// A runtime whose keys live in `keys_dir` (the config directory's `ai-keys`).
    pub fn new(keys_dir: PathBuf) -> Self {
        Self {
            keys: KeyStore::new(keys_dir),
            provider: Mutex::new(None),
            running: Arc::default(),
        }
    }

    /// The keys this device has saved.
    pub fn keys(&self) -> &KeyStore {
        &self.keys
    }

    /// Replaces the Provider every Run uses from now on (tests install scripted ones).
    #[cfg(test)]
    pub fn set_provider(&self, provider: Arc<dyn Provider>) {
        *lock(&self.provider) = Some(provider);
    }

    /// The adapter a Run asks: the test override if one is set, else the real one for the record.
    fn provider_for(&self, prepared: &Prepared) -> Arc<dyn Provider> {
        if let Some(provider) = lock(&self.provider).clone() {
            return provider;
        }
        mnemora_ai::build(
            &prepared.record,
            &prepared.choice.model,
            prepared.key.clone(),
        )
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

/// A question, the Provider and Model it goes to, and the Conversation (none starts one).
#[derive(Debug, Clone, Deserialize, Type)]
pub struct RunRequest {
    /// The Conversation to continue, or none to start one.
    pub conversation_id: Option<String>,
    /// The question as typed.
    pub text: String,
    /// The Provider that answers.
    pub provider_id: String,
    /// The Model id it is asked for.
    pub model: String,
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
    let prepared = mnemora_ai::prepare_run(
        &lock(&state.db),
        state.ai.keys(),
        &request.provider_id,
        &request.model,
    )?;
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
    let provider = state.ai.provider_for(&prepared);
    let (tools, off) = offered_tools(&state)?;
    let ask = mnemora_ai::Ask {
        conversation_id,
        continues,
        text: request.text,
        local: request.local,
        choice: prepared.choice,
    };
    // The question is saved before the command returns, so the Conversation can be read at once.
    let question_id = mnemora_ai::begin(&state.db, &ask)?;
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut slot = Some(slot);
        let state = handle.state::<AppState>();
        mnemora_ai::execute(
            &state.db,
            provider.as_ref(),
            &ask,
            &question_id,
            tools,
            &off,
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

/// Every Provider on this device, oldest first. Never carries a key.
#[tauri::command]
#[specta::specta]
pub fn ai_provider_list(state: State<'_, AppState>) -> Result<Vec<ProviderView>, AppError> {
    mnemora_ai::list_providers(&lock(&state.db), state.ai.keys())
}

/// Adds a Provider of the kind chosen first.
#[tauri::command]
#[specta::specta]
pub fn ai_provider_create(
    state: State<'_, AppState>,
    request: NewProvider,
) -> Result<ProviderView, AppError> {
    let conn = lock(&state.db);
    let now = physical_now(&conn);
    mnemora_ai::create_provider(&conn, state.ai.keys(), &request, now)
}

/// Changes a Provider's name and address. A new address clears its key.
#[tauri::command]
#[specta::specta]
pub fn ai_provider_update(
    state: State<'_, AppState>,
    id: String,
    request: ProviderUpdate,
) -> Result<ProviderView, AppError> {
    let conn = lock(&state.db);
    let now = physical_now(&conn);
    mnemora_ai::update_provider(&conn, state.ai.keys(), &id, &request, now)
}

/// Deletes a Provider and its key at once; the screen asks the person first.
#[tauri::command]
#[specta::specta]
pub fn ai_provider_delete(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let conn = lock(&state.db);
    let now = physical_now(&conn);
    mnemora_ai::delete_provider(&conn, state.ai.keys(), &id, now)
}

/// Saves a Provider's key. Write-only: the key is never returned or logged.
#[tauri::command]
#[specta::specta]
pub fn ai_provider_key_set(
    state: State<'_, AppState>,
    id: String,
    key: String,
) -> Result<ProviderView, AppError> {
    mnemora_ai::set_key(&lock(&state.db), state.ai.keys(), &id, &key)
}

/// Removes a Provider's key.
#[tauri::command]
#[specta::specta]
pub fn ai_provider_key_clear(
    state: State<'_, AppState>,
    id: String,
) -> Result<ProviderView, AppError> {
    mnemora_ai::clear_key(&lock(&state.db), state.ai.keys(), &id)
}

/// Asks a Provider whether it answers, and how many Models it offers.
#[tauri::command]
#[specta::specta]
pub async fn ai_provider_test(
    state: State<'_, AppState>,
    id: String,
) -> Result<ConnectionTest, AppError> {
    let (record, key) = mnemora_ai::reachable(&lock(&state.db), state.ai.keys(), &id)?;
    tauri::async_runtime::spawn_blocking(move || mnemora_ai::connection_test(&record, key))
        .await
        .map_err(join_error)
}

/// The Model ids a Provider offers that contain `query`. Asked only when the person browses or
/// searches; the screen keeps the answer for the session.
#[tauri::command]
#[specta::specta]
pub async fn ai_models_search(
    state: State<'_, AppState>,
    provider_id: String,
    query: String,
) -> Result<ModelList, AppError> {
    let (record, key) = mnemora_ai::reachable(&lock(&state.db), state.ai.keys(), &provider_id)?;
    tauri::async_runtime::spawn_blocking(move || mnemora_ai::matching_models(&record, key, &query))
        .await
        .map_err(join_error)
}

/// One page of the history, newest first, whose titles contain `query`. 30 at a time from the screen.
#[tauri::command]
#[specta::specta]
pub fn ai_conversation_list(
    state: State<'_, AppState>,
    query: String,
    offset: u32,
    limit: u32,
) -> Result<ConversationPage, AppError> {
    mnemora_ai::list_conversations(&lock(&state.db), &query, offset, limit)
}

/// Gives a Conversation a new title, checked first.
#[tauri::command]
#[specta::specta]
pub fn ai_conversation_rename(
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<(), AppError> {
    mnemora_ai::rename_conversation(&lock(&state.db), &id, &title)
}

/// Deletes a Conversation at once; Undo restores it. Refused while a Run answers in it, since that
/// Run would otherwise save into a deleted Conversation.
#[tauri::command]
#[specta::specta]
pub fn ai_conversation_delete(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    if state.ai.is_running(&id) {
        return Err(AppError::new(
            ErrorCode::Validation,
            "This Conversation is still answering.",
        ));
    }
    let conn = lock(&state.db);
    let now = physical_now(&conn);
    mnemora_ai::delete_conversation(&conn, &id, now)
}

/// Brings a deleted Conversation back, with every entry it had.
#[tauri::command]
#[specta::specta]
pub fn ai_conversation_restore(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    mnemora_ai::restore_conversation(&lock(&state.db), &id)
}

/// A Conversation's visible branch as Markdown, or with `entry_id` one answer, without its citations.
#[tauri::command]
#[specta::specta]
pub fn ai_conversation_markdown(
    state: State<'_, AppState>,
    id: String,
    entry_id: Option<String>,
) -> Result<String, AppError> {
    mnemora_ai::conversation_markdown(&lock(&state.db), &id, entry_id.as_deref())
}

/// The Saved questions, in their manual order.
#[tauri::command]
#[specta::specta]
pub fn ai_saved_list(state: State<'_, AppState>) -> Result<Vec<SavedQuestion>, AppError> {
    mnemora_ai::list_saved(&lock(&state.db))
}

/// Adds a Saved question at the end of the list.
#[tauri::command]
#[specta::specta]
pub fn ai_saved_create(
    state: State<'_, AppState>,
    text: String,
) -> Result<SavedQuestion, AppError> {
    let conn = lock(&state.db);
    let now = physical_now(&conn);
    mnemora_ai::create_saved(&conn, &text, now)
}

/// Changes a Saved question's text.
#[tauri::command]
#[specta::specta]
pub fn ai_saved_update(
    state: State<'_, AppState>,
    id: String,
    text: String,
) -> Result<SavedQuestion, AppError> {
    let conn = lock(&state.db);
    let now = physical_now(&conn);
    mnemora_ai::update_saved(&conn, &id, &text, now)
}

/// Deletes a Saved question at once; Undo restores it in its place.
#[tauri::command]
#[specta::specta]
pub fn ai_saved_delete(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let conn = lock(&state.db);
    let now = physical_now(&conn);
    mnemora_ai::delete_saved(&conn, &id, now)
}

/// Brings a deleted Saved question back in its place.
#[tauri::command]
#[specta::specta]
pub fn ai_saved_restore(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let conn = lock(&state.db);
    let now = physical_now(&conn);
    mnemora_ai::restore_saved(&conn, &id, now)
}

/// Puts a Saved question between two neighbours the screen showed (either may be none).
#[tauri::command]
#[specta::specta]
pub fn ai_saved_reorder(
    state: State<'_, AppState>,
    id: String,
    after_id: Option<String>,
    before_id: Option<String>,
) -> Result<(), AppError> {
    let conn = lock(&state.db);
    let now = physical_now(&conn);
    mnemora_ai::reorder_saved(&conn, &id, after_id.as_deref(), before_id.as_deref(), now)
}

/// The Models this device has used most recently and most often.
#[tauri::command]
#[specta::specta]
pub fn ai_models_usage(state: State<'_, AppState>) -> Result<ModelUsage, AppError> {
    mnemora_ai::model_usage(&lock(&state.db))
}

/// The Tools of every Module that is turned on, and the names of the Modules that are off and
/// would have offered some, for the prompt. Read from the Preferences that list the exceptions.
/// The Tools a Run may call, and the names of the Modules that are off and would have offered some.
type Offer = (Vec<Box<dyn Tool>>, Vec<String>);

fn offered_tools(state: &AppState) -> Result<Offer, AppError> {
    let conn = lock(&state.db);
    let disabled = preference_list(&conn, "disabled_modules")?;
    let opted_in = preference_list(&conn, "enabled_optional_modules")?;
    let mut tools = Vec::new();
    let mut off = Vec::new();
    for module in state.modules.iter() {
        if is_enabled(module.as_ref(), &disabled, &opted_in) {
            tools.extend(module.tools());
        } else if !module.tools().is_empty() {
            off.push(capitalised(module.id()));
        }
    }
    Ok((tools, off))
}

fn preference_list(
    conn: &mnemora_core::db::Connection,
    key: &str,
) -> Result<Vec<String>, AppError> {
    let stored = mnemora_core::settings::stored(conn, "core", key)?;
    Ok(stored
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default())
}

/// A Module's name as the person reads it: "journal" becomes "Journal".
fn capitalised(id: &str) -> String {
    let mut chars = id.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

fn join_error(error: tauri::Error) -> AppError {
    AppError::new(ErrorCode::Internal, "The AI service check stopped.").with_details(error)
}
