//! What the AI Module does with Providers: validate, store, keep keys in step with them and test
//! them. The commands in the app call these; they hold no rules of their own.

use mnemora_core::error::AppResult;
use mnemora_core::util::new_id;

use crate::error::AiError;
use crate::keys::KeyStore;
use crate::provider::{
    ConnectionTest, KeyPolicy, ModelList, ModelUsage, NewProvider, ProviderKind, ProviderRecord,
    ProviderUpdate, ProviderView, build_models, validate_address, validate_key, validate_model,
    validate_name,
};
use crate::provider_store as store;
use crate::run::{Choice, failure_message};

/// Every live Provider, oldest first, with whether its key is saved.
pub fn list(conn: &rusqlite::Connection, keys: &KeyStore) -> AppResult<Vec<ProviderView>> {
    store::list(conn)?
        .iter()
        .map(|record| Ok(record.view(keys.has(&record.id)?)))
        .collect()
}

/// Adds a Provider. The address is checked for its kind before anything is stored.
pub fn create(
    conn: &rusqlite::Connection,
    keys: &KeyStore,
    new: &NewProvider,
    now: i64,
) -> AppResult<ProviderView> {
    let record = ProviderRecord {
        id: new_id(),
        name: validate_name(&new.name)?,
        kind: new.kind,
        address: address_for(new.kind, &new.address)?,
        created_at: now,
        updated_at: now,
    };
    store::insert(conn, &record)?;
    Ok(record.view(keys.has(&record.id)?))
}

/// The address a Provider is saved with: the kind's fixed one (which the form may echo back, but
/// no other), or the address the person typed, checked for its kind.
fn address_for(kind: ProviderKind, typed: &str) -> AppResult<String> {
    match kind.fixed_address() {
        Some(fixed) => {
            let typed = typed.trim().trim_end_matches('/');
            if !typed.is_empty() && typed != fixed {
                return Err(AiError::Validation(
                    "This kind of Provider uses its own fixed address.",
                )
                .into());
            }
            Ok(fixed.to_owned())
        }
        None => Ok(validate_address(typed)?),
    }
}

/// Changes a Provider's name and address. A key is bound to the address it was saved with, so a
/// new address clears it.
pub fn update(
    conn: &rusqlite::Connection,
    keys: &KeyStore,
    id: &str,
    change: &ProviderUpdate,
    now: i64,
) -> AppResult<ProviderView> {
    let current = live(conn, id)?;
    let name = validate_name(&change.name)?;
    let address = address_for(current.kind, &change.address)?;
    if address != current.address {
        keys.remove(id)?;
    }
    store::update(conn, id, &name, &address, now)?;
    let updated = live(conn, id)?;
    Ok(updated.view(keys.has(id)?))
}

/// Deletes a Provider and its key. Conversations that named it keep their text.
pub fn delete(conn: &rusqlite::Connection, keys: &KeyStore, id: &str, now: i64) -> AppResult<()> {
    live(conn, id)?;
    keys.remove(id)?;
    store::delete(conn, id, now)
}

/// Saves a key for a Provider whose kind takes one. The key is never returned.
pub fn set_key(
    conn: &rusqlite::Connection,
    keys: &KeyStore,
    id: &str,
    key: &str,
) -> AppResult<ProviderView> {
    let record = live(conn, id)?;
    if record.kind.key_policy() == KeyPolicy::NotNeeded {
        return Err(AiError::Validation("This kind of Provider does not use a key.").into());
    }
    keys.set(id, &validate_key(key)?)?;
    Ok(record.view(true))
}

/// Removes a Provider's key.
pub fn clear_key(
    conn: &rusqlite::Connection,
    keys: &KeyStore,
    id: &str,
) -> AppResult<ProviderView> {
    let record = live(conn, id)?;
    keys.remove(id)?;
    Ok(record.view(false))
}

/// What a connection test or a Model search needs from the database: the Provider and its key.
/// Read under the lock, then used without it, since asking a Provider can take minutes.
pub fn reachable(
    conn: &rusqlite::Connection,
    keys: &KeyStore,
    id: &str,
) -> AppResult<(ProviderRecord, Option<String>)> {
    let record = live(conn, id)?;
    let key = keys.get(id)?;
    Ok((record, key))
}

/// Asks the Provider for its Models. Any answer means it is reachable; the message says which.
pub fn connection_test(record: &ProviderRecord, key: Option<String>) -> ConnectionTest {
    match build_models(record, key) {
        Ok(models) => ConnectionTest {
            connected: true,
            models: u32::try_from(models.len()).unwrap_or(u32::MAX),
            failure: None,
            message: format!("Connected · {} models", models.len()),
        },
        Err(failure) => ConnectionTest {
            connected: false,
            models: 0,
            failure: Some(failure),
            message: failure_message(failure).to_owned(),
        },
    }
}

/// The Model ids a Provider offers that contain `query` (all of them when it is empty). This
/// asks the Provider, so the screen calls it only when the person browses or searches.
pub fn matching_models(record: &ProviderRecord, key: Option<String>, query: &str) -> ModelList {
    match build_models(record, key) {
        Ok(models) => {
            let needle = query.trim().to_lowercase();
            ModelList {
                models: models
                    .into_iter()
                    .filter(|model| needle.is_empty() || model.to_lowercase().contains(&needle))
                    .collect(),
                failure: None,
                message: String::new(),
            }
        }
        Err(failure) => ModelList {
            models: Vec::new(),
            failure: Some(failure),
            message: failure_message(failure).to_owned(),
        },
    }
}

/// A question's Provider and Model, checked: the Provider is live, the Model is a valid id, and
/// a key is present if the kind needs one.
pub struct Prepared {
    /// The Provider that answers.
    pub record: ProviderRecord,
    /// Its saved key, if it has one.
    pub key: Option<String>,
    /// The Provider and Model, for the Conversation.
    pub choice: Choice,
}

/// Checks the Provider and Model a question is asked with.
pub fn prepare_run(
    conn: &rusqlite::Connection,
    keys: &KeyStore,
    provider_id: &str,
    model: &str,
) -> AppResult<Prepared> {
    let model = validate_model(model)?;
    let record = live(conn, provider_id)?;
    let key = match record.kind.key_policy() {
        KeyPolicy::NotNeeded => None,
        KeyPolicy::Optional => keys.get(provider_id)?,
        KeyPolicy::Required => Some(
            keys.get(provider_id)?
                .ok_or(AiError::Validation("Add this Provider's key first."))?,
        ),
    };
    Ok(Prepared {
        choice: Choice {
            provider_id: record.id.clone(),
            provider_name: record.name.clone(),
            model,
        },
        record,
        key,
    })
}

/// The Models this device has used: the latest and the most used.
pub fn usage(conn: &rusqlite::Connection) -> AppResult<ModelUsage> {
    Ok(ModelUsage {
        recent: store::usage(conn, false, 5)?,
        most_used: store::usage(conn, true, 5)?,
    })
}

/// A live Provider, or a "no longer exists" error.
pub fn live(conn: &rusqlite::Connection, id: &str) -> AppResult<ProviderRecord> {
    Ok(store::live(conn, id)?.ok_or(AiError::ProviderNotFound)?)
}
