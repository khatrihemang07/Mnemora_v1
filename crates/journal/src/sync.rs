//! The Journal's side of the sync contract for Entries (ADR 0017): which Entry fields merge how,
//! and how values received from another device are written. Entry text must never reach a log
//! line.
//!
//! - `body` is Text: newest wins for now, three-way merge later (the engine decides; this file
//!   only writes what it is given).
//! - The **Page date** and the **deleted** mark are ordinary values: moving an Entry on one
//!   device and editing it on another both stand, and a delete beats nothing by itself.
//! - The **written moment** (instant, UTC offset, zone) never changes after creation and is one
//!   atomic group, so a creation always arrives whole.
//! - `edited_at`, `word_count` and `updated_at` are derived and recomputed here from what
//!   arrived, never sent: the word count from the body, the edited time from when the origin
//!   made the change (a typo fix within ten minutes of writing is not an edit; a checklist tick
//!   never is).

use mnemora_core::changelog::{self, Change, ChangeOp, Unlogged};
use mnemora_core::db::physical_now;
use mnemora_core::error::AppResult;
use mnemora_core::search::plain;
use mnemora_core::syncable::{Applied, RemoteChange, SyncEntity, SyncEvent, SyncField};
use rusqlite::Connection;
use serde_json::{Map, Value};

use crate::entry::{Entry, validate_page_date, validate_text, validate_written_zone};
use crate::repo;
use crate::service::{EDIT_GRACE_MS, ENTITY, snapshot_of};

const WRITTEN: &str = "written";

/// Everything the Journal syncs.
pub(crate) const ENTITIES: &[SyncEntity] = &[ENTRY];

/// The Entry entity's fields and how each merges.
const ENTRY: SyncEntity = SyncEntity {
    name: ENTITY,
    fields: &[
        SyncField::text("body"),
        SyncField::value("page_date"),
        SyncField::value("written_at").in_group(WRITTEN),
        SyncField::value("written_offset_min").in_group(WRITTEN),
        SyncField::value("written_tz").in_group(WRITTEN),
        SyncField::value("created_at"),
        SyncField::value("deleted_at"),
        SyncField::derived("edited_at"),
        SyncField::derived("word_count"),
        SyncField::derived("updated_at"),
    ],
};

/// Logs every Entry (deleted ones included) whose state the change log lacks, stamped with the
/// time the row says it last changed (`Module::backfill_log`).
pub(crate) fn backfill(conn: &Connection) -> AppResult<()> {
    let mut unlogged = Vec::new();
    for (entry, deleted_at) in repo::every(conn)? {
        unlogged.push(Unlogged {
            change: Change {
                module: crate::MODULE_ID.into(),
                entity: ENTITY.into(),
                entity_id: entry.id.clone(),
                op: ChangeOp::Update,
                session_id: None,
                snapshot: snapshot_of(&entry, deleted_at)?,
            },
            ms: entry.updated_at,
            after: Vec::new(),
        });
    }
    changelog::backfill(conn, &ENTRY, unlogged)?;
    Ok(())
}

/// What a set of received fields would write, every unit already checked. A unit that fails its
/// checks (a bug or a hostile peer) is left out whole.
#[derive(Default)]
struct Patch {
    body: Option<String>,
    page_date: Option<String>,
    written: Option<(i64, i32, String)>,
    created_at: Option<i64>,
    deleted_at: Option<Deleted>,
}

/// A received deleted mark: when, or none for "not deleted".
struct Deleted(Option<i64>);

impl Patch {
    fn read(fields: &Map<String, Value>) -> Self {
        let text = |k: &str| fields.get(k).and_then(Value::as_str);
        Self {
            body: text("body")
                .filter(|b| validate_text(b).is_ok())
                .map(str::to_owned),
            page_date: text("page_date")
                .filter(|d| validate_page_date(d).is_ok())
                .map(str::to_owned),
            written: written_group(fields),
            created_at: fields.get("created_at").and_then(Value::as_i64),
            deleted_at: match fields.get("deleted_at") {
                Some(Value::Null) => Some(Deleted(None)),
                Some(v) => v.as_i64().map(|at| Deleted(Some(at))),
                None => None,
            },
        }
    }
}

fn written_group(fields: &Map<String, Value>) -> Option<(i64, i32, String)> {
    let at = fields.get("written_at")?.as_i64()?;
    let offset = i32::try_from(fields.get("written_offset_min")?.as_i64()?).ok()?;
    let tz = fields.get("written_tz")?.as_str()?;
    validate_written_zone(tz, offset).ok()?;
    Some((at, offset, tz.to_owned()))
}

/// Applies the winning field values of one Entry, creating it when it is new here.
pub(crate) fn apply(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let id = change.entity_id;
    let patch = Patch::read(change.fields);
    let before = repo::get_any(conn, id)?;
    let before_snapshot = before
        .as_ref()
        .map(|(entry, deleted)| snapshot_of(entry, *deleted))
        .transpose()?;

    let written_at = if let Some((entry, _)) = &before {
        entry.written_at
    } else {
        // An Entry can only be made from its first record, which carries all of these.
        let (Some(body), Some(page_date), Some((written_at, offset, tz)), Some(created_at)) = (
            &patch.body,
            &patch.page_date,
            &patch.written,
            patch.created_at,
        ) else {
            tracing::debug!(target: "sync", entry = id, "record for an unknown Entry without its creation, kept for relay only");
            return Ok(Applied {
                snapshot: Value::Object(change.fields.clone()),
                changed: false,
                events: Vec::new(),
            });
        };
        let now = physical_now(conn);
        repo::insert(
            conn,
            &Entry {
                id: id.to_owned(),
                page_date: page_date.clone(),
                word_count: plain::word_count(body),
                body: body.clone(),
                written_at: *written_at,
                written_offset_min: *offset,
                written_tz: tz.clone(),
                edited_at: edited_by(*written_at, change.at, None, change.op),
                created_at,
                updated_at: now,
            },
        )?;
        *written_at
    };

    write(conn, id, &patch, written_at, change)?;

    let (entry, deleted_at) = repo::get_any(conn, id)?.ok_or_else(|| {
        mnemora_core::error::AppError::new(
            mnemora_core::error::ErrorCode::Internal,
            "Entry vanished while syncing",
        )
    })?;
    let page_date = entry.page_date.clone();
    let snapshot = snapshot_of(&entry, deleted_at)?;
    let changed = before_snapshot.as_ref().map(strip_updated) != Some(strip_updated(&snapshot));
    let snapshot = if changed {
        // `updated_at` is derived: it says when this device last changed the row.
        let now = physical_now(conn);
        repo::update_columns(conn, id, &[("updated_at", &now)])?;
        snapshot_of(
            &Entry {
                updated_at: now,
                ..entry
            },
            deleted_at,
        )?
    } else {
        snapshot
    };
    // An Entry that was showing here and was deleted on another device: an open editor is told.
    let mut events = Vec::new();
    if before
        .as_ref()
        .is_some_and(|(_, deleted)| deleted.is_none())
        && deleted_at.is_some()
    {
        events.push(SyncEvent::EntryDeleted {
            entry_id: id.to_owned(),
            page_date,
            device: None,
        });
    }
    Ok(Applied {
        snapshot,
        changed,
        events,
    })
}

/// When an Entry counts as edited after a change the origin made at `at`: that moment, unless the
/// change was within the grace after writing (a typo fix) or a tick, which leave it as it was.
/// It is the moment of the change that wrote the text, so devices that took the same text show
/// the same time.
fn edited_by(written_at: i64, at: i64, current: Option<i64>, op: ChangeOp) -> Option<i64> {
    if op == ChangeOp::Tick || at - written_at <= EDIT_GRACE_MS {
        return current;
    }
    Some(at)
}

/// A snapshot without `updated_at`, which is derived and says nothing about what changed.
fn strip_updated(snapshot: &Value) -> Value {
    let mut copy = snapshot.clone();
    if let Some(fields) = copy.as_object_mut() {
        fields.remove("updated_at");
    }
    copy
}

/// Writes every checked unit of `patch`, and what derives from the body.
fn write(
    conn: &Connection,
    id: &str,
    patch: &Patch,
    written_at: i64,
    change: &RemoteChange<'_>,
) -> AppResult<()> {
    let mut sets: Vec<(&str, &dyn rusqlite::ToSql)> = Vec::new();
    let (word_count, edited_at);
    if let Some(body) = &patch.body
        && let Some((entry, _)) = repo::get_any(conn, id)?
        && entry.body != *body
    {
        word_count = plain::word_count(body);
        edited_at = edited_by(written_at, change.at, entry.edited_at, change.op);
        sets.push(("body", body));
        sets.push(("word_count", &word_count));
        sets.push(("edited_at", &edited_at));
    }
    if let Some(date) = &patch.page_date {
        sets.push(("page_date", date));
    }
    let written;
    if let Some((at, offset, tz)) = &patch.written {
        written = (at, offset, tz);
        sets.push(("written_at", written.0));
        sets.push(("written_offset_min", written.1));
        sets.push(("written_tz", written.2));
    }
    if let Some(created_at) = &patch.created_at {
        sets.push(("created_at", created_at));
    }
    if let Some(deleted_at) = &patch.deleted_at {
        sets.push(("deleted_at", &deleted_at.0));
    }
    repo::update_columns(conn, id, &sets)
}

#[cfg(test)]
mod tests {
    use mnemora_core::db::{self, migrate};
    use mnemora_core::module::CoreModule;
    use mnemora_core::settings::SettingsRegistry;
    use serde_json::json;

    use super::*;
    use crate::JournalModule;

    fn migrated() -> Connection {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        migrate::run(&mut conn, &[&CoreModule, &JournalModule], dir.path()).unwrap();
        conn
    }

    fn receive(conn: &Connection, id: &str, op: ChangeOp, fields: Value) -> Applied {
        let settings = SettingsRegistry::new();
        let fields = fields.as_object().unwrap().clone();
        apply(
            conn,
            &RemoteChange {
                entity: ENTITY,
                entity_id: id,
                op,
                at: 1_000,
                fields: &fields,
                settings: &settings,
            },
        )
        .unwrap()
    }

    fn whole(body: &str, date: &str) -> Value {
        json!({
            "body": body, "page_date": date, "written_at": 1_000, "written_offset_min": 330,
            "written_tz": "Asia/Kolkata", "created_at": 1_000, "deleted_at": null,
        })
    }

    #[test]
    fn an_entry_is_created_from_its_first_record_with_derived_fields_recomputed() {
        let conn = migrated();
        let applied = receive(
            &conn,
            "e1",
            ChangeOp::Create,
            whole("one **two** three", "2026-10-20"),
        );
        assert!(applied.changed);
        let (entry, deleted) = repo::get_any(&conn, "e1").unwrap().unwrap();
        assert_eq!(
            (entry.word_count, entry.edited_at, deleted),
            (3, None, None)
        );
        assert_eq!(entry.written_tz, "Asia/Kolkata");
    }

    #[test]
    fn what_a_peer_sends_is_checked_and_a_bad_unit_is_left_out_whole() {
        let conn = migrated();
        // Not a real date, or a blank body, so there is nothing to create the Entry from.
        let applied = receive(&conn, "e1", ChangeOp::Create, whole("text", "2026-02-30"));
        assert!(!applied.changed);
        assert!(repo::get_any(&conn, "e1").unwrap().is_none());
        let applied = receive(&conn, "e1", ChangeOp::Create, whole("   ", "2026-10-20"));
        assert!(!applied.changed);
        assert!(repo::get_any(&conn, "e1").unwrap().is_none());
        // On an Entry that exists, the bad unit is skipped and the good ones stand.
        receive(&conn, "e2", ChangeOp::Create, whole("kept", "2026-10-20"));
        receive(
            &conn,
            "e2",
            ChangeOp::Update,
            json!({"body": "\n", "page_date": "2026-10-21", "written_at": 5, "written_offset_min": 99_999, "written_tz": "UTC"}),
        );
        let (entry, _) = repo::get_any(&conn, "e2").unwrap().unwrap();
        assert_eq!(entry.body, "kept", "blank text refused");
        assert_eq!(entry.page_date, "2026-10-21");
        assert_eq!(
            entry.written_at, 1_000,
            "an impossible zone leaves the moment alone"
        );
    }

    #[test]
    fn a_record_for_an_unknown_entry_without_its_creation_is_kept_for_relay_only() {
        let conn = migrated();
        let applied = receive(
            &conn,
            "ghost",
            ChangeOp::Update,
            json!({"page_date": "2026-10-21"}),
        );
        assert!(!applied.changed);
        assert!(repo::get_any(&conn, "ghost").unwrap().is_none());
    }
}
