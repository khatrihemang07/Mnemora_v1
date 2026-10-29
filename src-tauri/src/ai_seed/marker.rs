//! The seeded marker: the row counts a finished seed records in its own table. Its presence, and
//! counts that still match, are what mark a database as holding the fixture.

use std::collections::BTreeMap;

use mnemora_core::db::Connection;

use super::database::{count_rows, table_exists};
use super::err;

/// The table a finished seed records its row counts in; its presence marks a seeded database.
const MARKER_TABLE: &str = "sandbox_seed";
/// The key the counts are recorded under in the marker table.
const MARKER_KEY: &str = "counts";
/// The key that says whether the seed left the catch-up day out of the fixture.
const FIXTURE_KEY: &str = "fixture";
/// The value of `FIXTURE_KEY` for a seed that left the catch-up day out.
const WITHOUT_CATCHUP: &str = "without-catchup";
/// The value of `FIXTURE_KEY` for a seed that wrote the whole fixture (the default).
const WHOLE: &str = "whole";

/// Counts the data tables and writes the counts to the marker table, which marks the database as
/// seeded. Only `inspect` reads it back.
pub(super) fn record_counts(conn: &Connection) -> Result<BTreeMap<String, i64>, String> {
    let counts = count_rows(conn)?;
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {MARKER_TABLE} (key TEXT PRIMARY KEY, value TEXT NOT NULL)"
    ))
    .map_err(err)?;
    let text = serde_json::to_string(&counts).map_err(err)?;
    conn.execute(
        &format!("INSERT OR REPLACE INTO {MARKER_TABLE} (key, value) VALUES (?1, ?2)"),
        (MARKER_KEY, text),
    )
    .map_err(err)?;
    Ok(counts)
}

/// Records whether the seed left the catch-up day out. Written after `record_counts`, which creates
/// the marker table.
pub(super) fn record_fixture(conn: &Connection, skip_catchup: bool) -> Result<(), String> {
    let value = if skip_catchup { WITHOUT_CATCHUP } else { WHOLE };
    conn.execute(
        &format!("INSERT OR REPLACE INTO {MARKER_TABLE} (key, value) VALUES (?1, ?2)"),
        (FIXTURE_KEY, value),
    )
    .map_err(err)?;
    Ok(())
}

/// Whether the seed that wrote this database left the catch-up day out. A database with no record of
/// it, or without the marker table, was written whole.
pub(super) fn leaves_out_catchup(conn: &Connection) -> Result<bool, String> {
    if !table_exists(conn, MARKER_TABLE)? {
        return Ok(false);
    }
    let mut statement = conn
        .prepare(&format!("SELECT value FROM {MARKER_TABLE} WHERE key = ?1"))
        .map_err(err)?;
    let mut found = statement
        .query_map([FIXTURE_KEY], |row| row.get::<_, String>(0))
        .map_err(err)?;
    match found.next() {
        None => Ok(false),
        Some(text) => Ok(text.map_err(err)? == WITHOUT_CATCHUP),
    }
}

/// The recorded counts, when the database carries the marker.
pub(super) fn marker(conn: &Connection) -> Result<Option<BTreeMap<String, i64>>, String> {
    if !table_exists(conn, MARKER_TABLE)? {
        return Ok(None);
    }
    let mut statement = conn
        .prepare(&format!("SELECT value FROM {MARKER_TABLE} WHERE key = ?1"))
        .map_err(err)?;
    let mut found = statement
        .query_map([MARKER_KEY], |row| row.get::<_, String>(0))
        .map_err(err)?;
    match found.next() {
        None => Ok(None),
        Some(text) => {
            let text = text.map_err(err)?;
            serde_json::from_str(&text).map(Some).map_err(err)
        }
    }
}
