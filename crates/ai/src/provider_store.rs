//! Reads and writes of Providers and the Model usage record. Rules live in `provider.rs`; this
//! file only moves rows. Providers are soft-deleted, so a Conversation that names one still reads.

use mnemora_core::error::{AppError, AppResult, ErrorCode};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::provider::{ModelUse, ProviderKind, ProviderRecord};

fn record(row: &Row<'_>) -> AppResult<ProviderRecord> {
    let kind: String = row.get(2)?;
    let kind = ProviderKind::parse(&kind).ok_or_else(|| {
        AppError::new(ErrorCode::Internal, "A stored Provider has an unknown kind")
    })?;
    Ok(ProviderRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        kind,
        address: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

const COLUMNS: &str = "id, name, kind, address, created_at, updated_at";

/// Adds a Provider.
pub fn insert(conn: &Connection, provider: &ProviderRecord) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ai_providers(id, name, kind, address, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            provider.id,
            provider.name,
            provider.kind.as_str(),
            provider.address,
            provider.created_at,
            provider.updated_at
        ],
    )?;
    Ok(())
}

/// A live Provider by id, if there is one.
pub fn live(conn: &Connection, id: &str) -> AppResult<Option<ProviderRecord>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM ai_providers WHERE id = ?1 AND deleted_at IS NULL"
    ))?;
    stmt.query_row([id], |row| Ok(record(row)))
        .optional()?
        .transpose()
}

/// Every live Provider, oldest first.
pub fn list(conn: &Connection) -> AppResult<Vec<ProviderRecord>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM ai_providers WHERE deleted_at IS NULL ORDER BY created_at, id"
    ))?;
    let rows = stmt.query_map([], |row| Ok(record(row)))?;
    rows.map(|row| row?).collect()
}

/// Changes a Provider's name and address.
pub fn update(conn: &Connection, id: &str, name: &str, address: &str, now: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_providers SET name = ?2, address = ?3, updated_at = ?4
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, name, address, now],
    )?;
    Ok(())
}

/// Marks a Provider deleted.
pub fn delete(conn: &Connection, id: &str, now: i64) -> AppResult<()> {
    conn.execute(
        "UPDATE ai_providers SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    Ok(())
}

/// Counts one question asked with `model` through `provider_id`.
pub fn record_use(conn: &Connection, provider_id: &str, model: &str, now: i64) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ai_model_usage(provider_id, model, uses, last_used_at) VALUES(?1, ?2, 1, ?3)
         ON CONFLICT(provider_id, model) DO UPDATE
         SET uses = uses + 1, last_used_at = excluded.last_used_at",
        params![provider_id, model, now],
    )?;
    Ok(())
}

/// The top `limit` uses among live Providers: by recency, or by how often.
pub fn usage(conn: &Connection, by_count: bool, limit: u32) -> AppResult<Vec<ModelUse>> {
    let order = if by_count {
        "u.uses DESC, u.last_used_at DESC"
    } else {
        "u.last_used_at DESC"
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT u.provider_id, p.name, u.model, u.uses, u.last_used_at
         FROM ai_model_usage u JOIN ai_providers p ON p.id = u.provider_id
         WHERE p.deleted_at IS NULL
         ORDER BY {order}, u.model LIMIT ?1"
    ))?;
    let rows = stmt.query_map([limit], |row| {
        Ok(ModelUse {
            provider_id: row.get(0)?,
            provider_name: row.get(1)?,
            model: row.get(2)?,
            uses: row.get(3)?,
            last_used_at: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
