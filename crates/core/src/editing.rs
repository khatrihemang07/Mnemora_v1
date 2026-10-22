//! Editing sessions that hold remote changes back (ADR 0017). While a person types in an Entry
//! or in one field of a Task, the screen has text of its own that Sync must not replace. The
//! editor takes a **hold** on what it edits and renews it while it stays open; Sync stores what
//! other devices send for held fields apart and applies it when the hold ends.
//!
//! A hold is a lease: it lapses [`LEASE_MS`] after the last renewal, so an editor that vanished
//! (app backgrounded, killed, webview crashed) cannot hold changes back for ever. The table is
//! cleared at every start, so a crash never survives into the next run. Local to this device,
//! never synced. Nothing here is user content.

use rusqlite::{Connection, params};

use crate::error::AppResult;

/// The scope that holds every field of the entity (a Journal Entry being edited).
pub const WHOLE: &str = "*";

/// How long a hold lasts without being renewed. Editors renew about every 30 s.
pub const LEASE_MS: i64 = 120_000;

/// Takes or renews the hold on `scope` (a field unit of the entity, or [`WHOLE`]) at `now`.
pub fn hold(
    conn: &Connection,
    entity: &str,
    entity_id: &str,
    scope: &str,
    now: i64,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO editing_sessions(entity, entity_id, scope, touched_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(entity, entity_id, scope) DO UPDATE SET touched_at = excluded.touched_at",
        params![entity, entity_id, scope, now],
    )?;
    Ok(())
}

/// Ends the hold on `scope`.
pub fn release(conn: &Connection, entity: &str, entity_id: &str, scope: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM editing_sessions WHERE entity = ?1 AND entity_id = ?2 AND scope = ?3",
        params![entity, entity_id, scope],
    )?;
    Ok(())
}

/// The scopes of `entity_id` that are held at `now` (a lapsed lease does not count).
pub fn held(conn: &Connection, entity: &str, entity_id: &str, now: i64) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT scope FROM editing_sessions
         WHERE entity = ?1 AND entity_id = ?2 AND touched_at > ?3",
    )?;
    let scopes = stmt
        .query_map(params![entity, entity_id, now - LEASE_MS], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(scopes)
}

/// Whether `unit` of the entity is held at `now`: its own scope or [`WHOLE`].
pub fn is_held(scopes: &[String], unit: &str) -> bool {
    scopes.iter().any(|s| s == WHOLE || s == unit)
}

/// Drops every hold: no editor survives a restart.
pub fn clear(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM editing_sessions", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, migrate};
    use crate::module::CoreModule;

    fn migrated() -> Connection {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        migrate::run(&mut conn, &[&CoreModule], dir.path()).unwrap();
        conn
    }

    #[test]
    fn a_hold_lasts_until_released_or_its_lease_lapses() {
        let conn = migrated();
        hold(&conn, "e", "1", "title", 1_000).unwrap();
        assert_eq!(held(&conn, "e", "1", 1_000).unwrap(), ["title"]);
        assert!(held(&conn, "e", "2", 1_000).unwrap().is_empty());
        // Renewing moves the lease on.
        hold(&conn, "e", "1", "title", 100_000).unwrap();
        assert_eq!(held(&conn, "e", "1", 200_000).unwrap(), ["title"]);
        assert!(
            held(&conn, "e", "1", 100_000 + LEASE_MS)
                .unwrap()
                .is_empty()
        );
        release(&conn, "e", "1", "title").unwrap();
        assert!(held(&conn, "e", "1", 100_000).unwrap().is_empty());
    }

    #[test]
    fn the_whole_scope_holds_every_unit_and_a_restart_drops_all() {
        let conn = migrated();
        hold(&conn, "e", "1", WHOLE, 5).unwrap();
        let scopes = held(&conn, "e", "1", 5).unwrap();
        assert!(is_held(&scopes, "body") && is_held(&scopes, "page_date"));
        assert!(!is_held(&["notes".into()], "title"));
        clear(&conn).unwrap();
        assert!(held(&conn, "e", "1", 5).unwrap().is_empty());
    }
}
