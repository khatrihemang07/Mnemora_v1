//! The shapes the AI Module sends across IPC. Field names are `snake_case`, as the Journal's are.

use mnemora_harness::{Failure, StopReason};
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
    /// The Model's answer.
    Assistant,
    /// The Provider and Model the following questions use.
    ModelChange,
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
    /// The Model's answer, with the Provider and Model that gave it, kept as text so another
    /// device can show them.
    Assistant {
        /// The answer's Markdown.
        text: String,
        /// How the reply ended.
        stop: StopReason,
        /// The name of the Provider that answered.
        provider: String,
        /// The Model id that answered.
        model: String,
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
    /// The Provider the Conversation last used, by name, as text (other devices may not have it).
    pub provider_name: Option<String>,
    /// The Model the Conversation last used.
    pub model: Option<String>,
    /// Whether a Run is answering in it on this device right now.
    pub running: bool,
    /// Every entry, oldest first.
    pub entries: Vec<EntryView>,
}

/// How a Run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RunOutcome {
    /// The answer was saved.
    Answered,
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
    /// The Run is over.
    RunEnded {
        /// How it ended.
        outcome: RunOutcome,
    },
}
