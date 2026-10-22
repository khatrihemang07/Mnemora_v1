//! The paired devices.

use mnemora_core::error::AppResult;
use rusqlite::Connection;

/// How many other devices this one is paired with.
pub fn paired_count(conn: &Connection) -> AppResult<u32> {
    Ok(conn.query_row("SELECT COUNT(*) FROM sync_peers", [], |r| r.get(0))?)
}

/// Forgets every paired device. Synced data stays; this device just leaves the group.
pub fn forget_all(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM sync_peers", [])?;
    tracing::info!(target: "sync", "left the group, all paired devices forgotten");
    Ok(())
}
