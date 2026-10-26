//! The shapes the AI Module sends across IPC. Field names are `snake_case`, as the Journal's are.

use mnemora_harness::{Failure, StopReason};

use crate::steering::QueuedNote;
use serde::{Deserialize, Serialize};
use specta::Type;

/// The device's calendar, sent with every question so that "last week" means what the person means.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct LocalContext {
    /// Today on the device, `YYYY-MM-DD`.
    pub today: String,
    /// Today's weekday in English, such as "Sunday".
    pub weekday: String,
    /// The device's IANA time zone, such as "Europe/London".
    pub time_zone: String,
    /// The first day of the week: `mon` or `sun`.
    pub week_start: String,
}

/// Who wrote an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// The person's question.
    User,
    /// The Model's answer, or a Step that asked for Tools.
    Assistant,
    /// What a Tool returned for one call.
    ToolResult,
    /// A correction turn the Model was sent.
    Correction,
    /// The Provider and Model the following questions use.
    ModelChange,
}

/// A Tool call the Model made, as it was sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CallRecord {
    /// The call's id.
    pub id: String,
    /// The Tool's name.
    pub name: String,
    /// The arguments, as the Model sent them (JSON text).
    pub arguments: String,
}

/// An Entry or Task an answer cites, with what the person needs to find it again. Kept in the
/// answer so another device can show it without the Tool results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Source {
    /// The Entry's or Task's id.
    pub id: String,
    /// The Module it belongs to, `journal` or `todo`.
    pub module: String,
    /// A label the person recognises it by.
    pub title: String,
    /// Its date (a Page date or a due date), when it has one.
    pub date: Option<String>,
}

/// What an entry holds. The tag is the entry's kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntryPayload {
    /// The person's question.
    User {
        /// The question as typed.
        text: String,
    },
    /// The Model's reply in one Step: the answer when it asks for no Tools, or the Tools it asked
    /// for. The answer carries the Provider and Model as text, so another device can show them.
    Assistant {
        /// The reply's Markdown.
        text: String,
        /// How the reply ended.
        stop: StopReason,
        /// The name of the Provider that answered.
        provider: String,
        /// The Model id that answered.
        model: String,
        /// The Tool calls this Step made, in order.
        calls: Vec<CallRecord>,
        /// The Entries and Tasks the answer cites, which a Tool returned in this Run.
        sources: Vec<Source>,
        /// Why the answer may be incomplete, when the Run ran out of steps.
        budget_note: Option<String>,
    },
    /// What a Tool returned for one call.
    ToolResult {
        /// The call it answers.
        call_id: String,
        /// The Tool's name.
        name: String,
        /// The arguments the Model sent (JSON text).
        arguments: String,
        /// What the Model read back, labelled with its observation id.
        result: String,
        /// Whether the call was refused.
        is_error: bool,
        /// The observation id this result is known by in the Run ("e1").
        observation_id: String,
        /// The plain line the activity shows for it.
        summary: String,
    },
    /// A correction the Model was sent, such as a citation no Tool returned.
    Correction {
        /// What the Model was told.
        text: String,
    },
    /// The Conversation switched to another Provider and Model. Carries no text for the Model.
    ModelChange {
        /// The name of the Provider chosen.
        provider_name: String,
        /// The Model id chosen.
        model: String,
    },
}

/// One entry of a Conversation's tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct EntryView {
    /// The entry's id.
    pub id: String,
    /// The entry this one followed, or none for the first question.
    pub parent_id: Option<String>,
    /// When it was written, in milliseconds. Typed as a number for TypeScript (ids and instants
    /// stay below 2^53); specta refuses `i64`.
    #[specta(type = f64)]
    pub created_at: i64,
    /// What it holds.
    pub payload: EntryPayload,
}

/// A Conversation and its entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ConversationView {
    /// The Conversation's id.
    pub id: String,
    /// Its title: the first question, trimmed.
    pub title: String,
    /// When it was started, in milliseconds (a number for TypeScript, as for entries).
    #[specta(type = f64)]
    pub created_at: i64,
    /// When it last changed, in milliseconds.
    #[specta(type = f64)]
    pub updated_at: i64,
    /// The Provider the Conversation last used, on this device.
    pub provider_id: Option<String>,
    /// The Provider the Conversation last used, by name, as text (other devices may not have it).
    pub provider_name: Option<String>,
    /// The Model the Conversation last used.
    pub model: Option<String>,
    /// Whether a Run is answering in it on this device right now.
    pub running: bool,
    /// The notes waiting for that Run, in the order they will be sent. Empty when none is answering.
    pub queued: Vec<QueuedNote>,
    /// Every entry, oldest first.
    pub entries: Vec<EntryView>,
}

/// How a Run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RunOutcome {
    /// The answer was saved.
    Answered,
    /// The person stopped the Run. What was written before is saved, marked as stopped.
    Stopped,
    /// No answer was saved. `message` is one plain sentence; the raw Provider error is never kept.
    Failed {
        /// Why.
        kind: Failure,
        /// What the person should read.
        message: String,
    },
}

/// One step of a Run, in the order it happens. The app wraps each with its Run and Conversation ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RunEvent {
    /// The Run began.
    RunStarted,
    /// An entry was written.
    EntrySaved {
        /// The entry's id.
        entry_id: String,
        /// Who wrote it.
        kind: EntryKind,
    },
    /// The Model is asked.
    StepStarted,
    /// A piece of the answer.
    TextDelta {
        /// The text, as streamed.
        text: String,
    },
    /// A Tool call is running.
    ToolStarted {
        /// The call's id.
        call_id: String,
        /// The Tool's name.
        name: String,
    },
    /// A Tool call finished.
    ToolEnded {
        /// The call's id.
        call_id: String,
        /// The Tool's name.
        name: String,
        /// The plain line the activity shows.
        summary: String,
        /// Whether the call was refused.
        is_error: bool,
    },
    /// The notes waiting to be sent to the Model changed. The last one sent after a Run ends
    /// lists the notes that were never sent, so the person can still send them.
    QueueChanged {
        /// The notes waiting, in the order they will be sent.
        notes: Vec<QueuedNote>,
    },
    /// The Run is over.
    RunEnded {
        /// How it ended.
        outcome: RunOutcome,
    },
}

/// One Conversation in the history: enough to list it and to open it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ConversationSummary {
    /// The Conversation's id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// When it was started, in milliseconds (a number for TypeScript, as for entries).
    #[specta(type = f64)]
    pub created_at: i64,
    /// When it last changed, in milliseconds; the history is newest first by this.
    #[specta(type = f64)]
    pub updated_at: i64,
}

/// One page of the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ConversationPage {
    /// The Conversations on this page, newest first.
    pub items: Vec<ConversationSummary>,
    /// Whether another page follows this one.
    pub more: bool,
}

/// A Saved question: a question the person runs with one tap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SavedQuestion {
    /// The Saved question's id.
    pub id: String,
    /// The question as it is asked.
    pub text: String,
}
