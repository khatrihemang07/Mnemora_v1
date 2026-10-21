//! The app-wide change log (ADR 0008): full JSON snapshots of changed entities, one record per
//! editing session. A Module may read its own records to build a readable history (Todo's Task
//! History); the raw log itself is never sent over IPC.
//!
//! Snapshots hold user content, so nothing in this file may log them.

use rusqlite::Connection;
use serde_json::Value;

use crate::error::AppResult;
use crate::util::{new_id, now_ms};

/// What happened to the entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeOp {
    /// The entity was created.
    Create,
    /// The entity was edited.
    Update,
    /// The entity was moved, e.g. a Journal Entry to another Page.
    Move,
    /// A checklist item inside the entity was ticked or unticked.
    Tick,
    /// The entity was deleted (soft or hard); the snapshot is its last state.
    Delete,
    /// A deleted entity was brought back.
    Restore,
}

impl ChangeOp {
    /// The value stored in the `op` column.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Move => "move",
            Self::Tick => "tick",
            Self::Delete => "delete",
            Self::Restore => "restore",
        }
    }
}

/// One change to record.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    /// Id of the owning Module.
    pub module: String,
    /// Kind of entity, e.g. `journal_entry`.
    pub entity: String,
    /// Id of the changed entity.
    pub entity_id: String,
    /// What happened.
    pub op: ChangeOp,
    /// Editing session. Changes sharing an entity and session collapse into one record.
    pub session_id: Option<String>,
    /// Full state of the entity after the change.
    pub snapshot: Value,
}

/// Records `change`. With a session id, the session's existing record for this entity is
/// replaced (snapshot and time; a `create` stays a `create`); otherwise a new record is appended.
/// Pass a transaction to make it atomic with the entity write.
pub fn record(conn: &Connection, change: &Change) -> AppResult<()> {
    conn.execute(
        "INSERT INTO change_log(id, module, entity, entity_id, op, session_id, snapshot, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(entity, entity_id, session_id) WHERE session_id IS NOT NULL
         DO UPDATE SET op = CASE WHEN op = 'create' THEN op ELSE excluded.op END, snapshot = excluded.snapshot, at = excluded.at",
        rusqlite::params![
            new_id(),
            change.module,
            change.entity,
            change.entity_id,
            change.op.as_str(),
            change.session_id,
            change.snapshot.to_string(),
            now_ms(),
        ],
    )?;
    Ok(())
}

/// One stored record, as read back.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// What happened.
    pub op: String,
    /// The editing session, if the change had one.
    pub session_id: Option<String>,
    /// Full state of the entity after the change.
    pub snapshot: Value,
    /// When it was recorded: UTC milliseconds. A session's record carries its latest edit.
    pub at: i64,
}

/// Every record of one entity, oldest first. A record whose snapshot cannot be read back is
/// skipped: the log must never stop a screen from loading.
pub fn read(conn: &Connection, entity: &str, entity_id: &str) -> AppResult<Vec<Record>> {
    let mut stmt = conn.prepare(
        "SELECT op, session_id, snapshot, at FROM change_log
         WHERE entity = ?1 AND entity_id = ?2 ORDER BY at, rowid",
    )?;
    let rows = stmt.query_map([entity, entity_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (op, session_id, snapshot, at) = row?;
        if let Ok(snapshot) = serde_json::from_str(&snapshot) {
            out.push(Record {
                op,
                session_id,
                snapshot,
                at,
            });
        }
    }
    Ok(out)
}

/// Deletes all history of the given entities of one kind. Returns how many records went.
pub fn purge(conn: &Connection, entity: &str, entity_ids: &[String]) -> AppResult<usize> {
    let mut stmt = conn.prepare("DELETE FROM change_log WHERE entity = ?1 AND entity_id = ?2")?;
    let mut removed = 0;
    for id in entity_ids {
        removed += stmt.execute([entity, id.as_str()])?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::{self, migrate};
    use crate::module::{CoreModule, Module};

    fn migrated() -> Connection {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        migrate::run(&mut conn, &[&CoreModule], dir.path()).unwrap();
        conn
    }

    fn change(entity_id: &str, session: Option<&str>, snapshot: Value) -> Change {
        Change {
            module: "journal".into(),
            entity: "entry".into(),
            entity_id: entity_id.into(),
            op: ChangeOp::Update,
            session_id: session.map(str::to_owned),
            snapshot,
        }
    }

    fn snapshots(conn: &Connection, entity_id: &str) -> Vec<Value> {
        let mut stmt = conn
            .prepare("SELECT snapshot FROM change_log WHERE entity_id = ?1 ORDER BY id")
            .unwrap();
        stmt.query_map([entity_id], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|s| serde_json::from_str(&s.unwrap()).unwrap())
            .collect()
    }

    #[test]
    fn changes_without_a_session_append() {
        let conn = migrated();
        record(&conn, &change("e1", None, json!({"v": 1}))).unwrap();
        record(&conn, &change("e1", None, json!({"v": 2}))).unwrap();
        assert_eq!(
            snapshots(&conn, "e1"),
            vec![json!({"v": 1}), json!({"v": 2})]
        );
    }

    #[test]
    fn same_session_collapses_to_the_latest_snapshot_and_keeps_create() {
        let conn = migrated();
        let mut first = change("e1", Some("s1"), json!({"v": 1}));
        first.op = ChangeOp::Create;
        record(&conn, &first).unwrap();
        record(&conn, &change("e1", Some("s1"), json!({"v": 2}))).unwrap();
        record(&conn, &change("e1", Some("s1"), json!({"v": 3}))).unwrap();
        assert_eq!(snapshots(&conn, "e1"), vec![json!({"v": 3})]);
        let op: String = conn
            .query_row("SELECT op FROM change_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            op, "create",
            "an Entry created and edited in one session was created"
        );
    }

    #[test]
    fn different_sessions_and_entities_append() {
        let conn = migrated();
        record(&conn, &change("e1", Some("s1"), json!({"v": 1}))).unwrap();
        record(&conn, &change("e1", Some("s2"), json!({"v": 2}))).unwrap();
        record(&conn, &change("e2", Some("s1"), json!({"v": 3}))).unwrap();
        assert_eq!(snapshots(&conn, "e1").len(), 2);
        assert_eq!(snapshots(&conn, "e2").len(), 1);
    }

    #[test]
    fn read_returns_one_entity_oldest_first() {
        let conn = migrated();
        record(&conn, &change("e1", None, json!({"v": 1}))).unwrap();
        record(&conn, &change("e2", None, json!({"v": 9}))).unwrap();
        record(&conn, &change("e1", Some("s1"), json!({"v": 2}))).unwrap();
        let read = read(&conn, "entry", "e1").unwrap();
        let seen: Vec<_> = read.iter().map(|r| r.snapshot.clone()).collect();
        assert_eq!(seen, vec![json!({"v": 1}), json!({"v": 2})]);
        assert_eq!(read[1].session_id.as_deref(), Some("s1"));
        assert!(super::read(&conn, "entry", "nope").unwrap().is_empty());
    }

    #[test]
    fn purge_removes_only_the_given_entities() {
        let conn = migrated();
        record(&conn, &change("e1", None, json!({}))).unwrap();
        record(&conn, &change("e1", Some("s1"), json!({}))).unwrap();
        record(&conn, &change("e2", None, json!({}))).unwrap();
        record(&conn, &change("e3", None, json!({}))).unwrap();
        let mut other_kind = change("e1", None, json!({}));
        other_kind.entity = "tag".into();
        record(&conn, &other_kind).unwrap();

        let removed = purge(&conn, "entry", &["e1".into(), "e2".into()]).unwrap();
        assert_eq!(removed, 3);
        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM change_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 2, "e3 and the other entity kind survive");
    }

    #[test]
    fn upgrading_an_existing_database_keeps_settings() {
        struct CoreV1;
        impl Module for CoreV1 {
            fn id(&self) -> &'static str {
                "core"
            }
            fn migrations(&self) -> rusqlite_migration::Migrations<'static> {
                rusqlite_migration::Migrations::new(vec![rusqlite_migration::M::up(
                    "CREATE TABLE settings(
                         module TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL,
                         updated_at INTEGER NOT NULL, PRIMARY KEY(module, key)
                     ) STRICT",
                )])
            }
            fn register_settings(&self, _: &mut crate::settings::SettingsRegistry) {}
        }

        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        migrate::run(&mut conn, &[&CoreV1], dir.path()).unwrap();
        conn.execute(
            "INSERT INTO settings(module, key, value, updated_at) VALUES ('core', 'theme', '\"dark\"', 1)",
            [],
        )
        .unwrap();

        let report = migrate::run(&mut conn, &[&CoreModule], dir.path()).unwrap();
        assert_eq!(report.applied, vec![("core".into(), 1, 2)]);
        assert!(report.backup.is_some(), "existing data is backed up first");
        let theme: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'theme'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(theme, "\"dark\"");
        record(&conn, &change("e1", None, json!({}))).unwrap();
    }
}
