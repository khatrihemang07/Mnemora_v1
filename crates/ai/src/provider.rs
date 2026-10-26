//! Providers: the services a device asks, their kinds and the rules for their fields. A Provider
//! belongs to one device and is never synced; its key lives in `keys.rs`, never in this record.

use std::sync::Arc;

use mnemora_harness::{Failure, Provider};
use mnemora_providers::{AnthropicProvider, CompatProvider, WrapperProvider, address, anthropic};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::AiError;

/// Longest name a Provider may have, in characters.
pub const NAME_CHARS: usize = 40;
/// Longest key accepted, in bytes.
pub const KEY_BYTES: usize = 512;
/// Longest Model id accepted, in characters.
pub const MODEL_CHARS: usize = 200;

/// The kinds of Provider. Each has fixed behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// The Local AI Wrapper: on this machine or the person's network, no key. Answers in the
    /// JSON envelope.
    Wrapper,
    /// Any endpoint that speaks the OpenAI chat format with native Tool calls, at an address the
    /// person gives. A key is optional.
    #[serde(rename = "openai_compatible")]
    OpenAiCompatible,
    /// Anthropic's Messages API: a fixed address and a key that is required.
    Anthropic,
}

/// Whether a kind takes a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPolicy {
    /// The kind never takes one.
    NotNeeded,
    /// The kind may take one.
    Optional,
    /// The kind cannot work without one.
    Required,
}

impl ProviderKind {
    /// The name shown for the kind.
    pub fn label(self) -> &'static str {
        match self {
            Self::Wrapper => "Local AI Wrapper",
            Self::OpenAiCompatible => "OpenAI-compatible",
            Self::Anthropic => "Anthropic",
        }
    }

    /// Whether a key is needed, allowed or not used.
    pub fn key_policy(self) -> KeyPolicy {
        match self {
            Self::Wrapper => KeyPolicy::NotNeeded,
            Self::OpenAiCompatible => KeyPolicy::Optional,
            Self::Anthropic => KeyPolicy::Required,
        }
    }

    /// The address this kind always uses, if it has one. Its Providers cannot be given another.
    pub fn fixed_address(self) -> Option<&'static str> {
        match self {
            Self::Anthropic => Some(anthropic::ADDRESS),
            Self::Wrapper | Self::OpenAiCompatible => None,
        }
    }

    /// The text stored for the kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wrapper => "wrapper",
            Self::OpenAiCompatible => "openai_compatible",
            Self::Anthropic => "anthropic",
        }
    }

    /// The kind a stored text names, if it is one this build knows.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "wrapper" => Some(Self::Wrapper),
            "openai_compatible" => Some(Self::OpenAiCompatible),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }
}

/// What the screen may learn about a key: only whether one is needed and whether one is saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum KeyState {
    /// The kind does not use a key.
    NotNeeded,
    /// A key is saved on this device.
    Saved,
    /// The kind needs a key and none is saved.
    Missing,
}

/// A Provider as stored on this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRecord {
    /// The Provider's id.
    pub id: String,
    /// The name the person gave it.
    pub name: String,
    /// Its kind.
    pub kind: ProviderKind,
    /// Its address, as saved (checked by `validate_address`).
    pub address: String,
    /// When it was added, in milliseconds.
    pub created_at: i64,
    /// When it last changed, in milliseconds.
    pub updated_at: i64,
}

/// A Provider as the screen sees it. Never carries the key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ProviderView {
    /// The Provider's id.
    pub id: String,
    /// The name the person gave it.
    pub name: String,
    /// Its kind.
    pub kind: ProviderKind,
    /// Its address.
    pub address: String,
    /// Whether a key is needed and saved.
    pub key_state: KeyState,
    /// When it was added, in milliseconds.
    #[specta(type = f64)]
    pub created_at: i64,
    /// When it last changed, in milliseconds.
    #[specta(type = f64)]
    pub updated_at: i64,
}

/// The fields a new Provider is made from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
pub struct NewProvider {
    /// Its kind, chosen first.
    pub kind: ProviderKind,
    /// The name the person gave it.
    pub name: String,
    /// Its address.
    pub address: String,
}

/// The fields a Provider's details may change to.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
pub struct ProviderUpdate {
    /// The new name.
    pub name: String,
    /// The new address.
    pub address: String,
}

/// The Models a Provider offers that match a search, or why it could not be asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ModelList {
    /// The matching Model ids, in the Provider's order.
    pub models: Vec<String>,
    /// Why the Provider could not be asked, if it could not.
    pub failure: Option<Failure>,
    /// What to show the person when there is a failure.
    pub message: String,
}

/// A Provider's connection test: what the Provider said, in one sentence the person can read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ConnectionTest {
    /// Whether the Provider answered.
    pub connected: bool,
    /// How many Models it offers, when it answered.
    pub models: u32,
    /// Why it did not answer, when it did not.
    pub failure: Option<Failure>,
    /// What to show the person.
    pub message: String,
}

impl ProviderRecord {
    /// The screen's view of this Provider, given whether its key is saved.
    pub fn view(&self, key_saved: bool) -> ProviderView {
        // An optional key that is not saved means the kind works without one.
        let key_state = match (self.kind.key_policy(), key_saved) {
            (KeyPolicy::NotNeeded, _) | (KeyPolicy::Optional, false) => KeyState::NotNeeded,
            (KeyPolicy::Optional | KeyPolicy::Required, true) => KeyState::Saved,
            (KeyPolicy::Required, false) => KeyState::Missing,
        };
        ProviderView {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind,
            address: self.address.clone(),
            key_state,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// A name, trimmed, of at most [`NAME_CHARS`] characters.
pub fn validate_name(raw: &str) -> Result<String, AiError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(AiError::Validation("Give this Provider a name."));
    }
    if name.chars().count() > NAME_CHARS {
        return Err(AiError::Validation("Keep the name to 40 characters."));
    }
    Ok(name.to_owned())
}

/// An address the kind may use, trimmed and without a trailing slash.
pub fn validate_address(raw: &str) -> Result<String, AiError> {
    address::check(raw).map_err(|e| match e {
        address::AddressError::Invalid => {
            AiError::Validation("Enter a full address, such as http://127.0.0.1:8080/v1.")
        }
        address::AddressError::UnsupportedScheme => {
            AiError::Validation("Use an http or https address.")
        }
        address::AddressError::PlainHttpNotAllowed => AiError::Validation(
            "Plain http works only on this machine, your local network and Tailscale. Use https for other addresses.",
        ),
    })
}

/// A key, trimmed. Empty and oversized keys are refused.
pub fn validate_key(raw: &str) -> Result<String, AiError> {
    let key = raw.trim();
    if key.is_empty() {
        return Err(AiError::Validation("Paste the key first."));
    }
    if key.len() > KEY_BYTES {
        return Err(AiError::Validation("That key is too long."));
    }
    Ok(key.to_owned())
}

/// A Model id, trimmed. Any id the Provider accepts is allowed, so only its shape is checked.
pub fn validate_model(raw: &str) -> Result<String, AiError> {
    let model = raw.trim();
    if model.is_empty() {
        return Err(AiError::Validation("Choose a Model first."));
    }
    if model.chars().count() > MODEL_CHARS || model.chars().any(char::is_control) {
        return Err(AiError::Validation("That Model id is not valid."));
    }
    Ok(model.to_owned())
}

/// The key a kind cannot work without. Without one the service would refuse the request anyway.
fn required(key: Option<String>) -> Result<String, Failure> {
    key.ok_or(Failure::KeyRejected)
}

/// The Model ids a stored Provider offers, asked with its saved key if it has one.
pub fn build_models(record: &ProviderRecord, key: Option<String>) -> Result<Vec<String>, Failure> {
    match record.kind {
        ProviderKind::Wrapper => {
            WrapperProvider::new(&record.name, "", &record.address, key).list_models()
        }
        ProviderKind::OpenAiCompatible => {
            CompatProvider::new(&record.name, "", &record.address, key).list_models()
        }
        ProviderKind::Anthropic => {
            AnthropicProvider::new(&record.name, "", &record.address, &required(key)?).list_models()
        }
    }
}

/// The adapter for a stored Provider, asking for `model`. `key` is the saved key, if any.
pub fn build(record: &ProviderRecord, model: &str, key: Option<String>) -> Arc<dyn Provider> {
    match record.kind {
        ProviderKind::Wrapper => Arc::new(WrapperProvider::new(
            &record.name,
            model,
            &record.address,
            key,
        )),
        ProviderKind::OpenAiCompatible => Arc::new(CompatProvider::new(
            &record.name,
            model,
            &record.address,
            key,
        )),
        // A run is never prepared without its key (`prepare_run`); a missing one is sent as none.
        ProviderKind::Anthropic => Arc::new(AnthropicProvider::new(
            &record.name,
            model,
            &record.address,
            key.as_deref().unwrap_or_default(),
        )),
    }
}

/// One Model a person has used, with the Provider it went through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ModelUse {
    /// The Provider's id.
    pub provider_id: String,
    /// The Provider's name, as it is now.
    pub provider_name: String,
    /// The Model id.
    pub model: String,
    /// How many questions were asked with it on this device.
    pub uses: u32,
    /// When it was last used, in milliseconds.
    #[specta(type = f64)]
    pub last_used_at: i64,
}

/// The Models a device has used: the most recent first, and the most used first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ModelUsage {
    /// The latest uses, newest first.
    pub recent: Vec<ModelUse>,
    /// The most used Models, most uses first.
    pub most_used: Vec<ModelUse>,
}
