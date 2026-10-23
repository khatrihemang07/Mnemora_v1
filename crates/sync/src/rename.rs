//! Naming the devices of the group. A device's name is the one value everybody shares, so a
//! rename (of this device in Settings, or of a paired device from its sheet) is a record of the
//! change-log entity `sync_device` (id: the device renamed, one Value field `name`) and the newest
//! wins. Names are unique across the group, ignoring case, accents and spacing.

use mnemora_core::changelog::{self, Change, ChangeOp};
use mnemora_core::error::AppResult;
use mnemora_core::hlc;
use mnemora_core::syncable::{Applied, RemoteChange, SyncEntity, SyncEvent, SyncField};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::error::SyncError;
use crate::name::validate_name;
use crate::peers::unique_name;

/// The change-log entity name.
pub const ENTITY: &str = "sync_device";

pub(crate) const DEVICE: SyncEntity = SyncEntity {
    name: ENTITY,
    fields: &[SyncField::value("name")],
};

/// Records that `device` is now called `name`, so the other devices take it.
fn record(conn: &Connection, device: &str, name: &str) -> AppResult<()> {
    let existed: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM change_log WHERE entity = ?1 AND entity_id = ?2)",
        params![ENTITY, device],
        |r| r.get(0),
    )?;
    changelog::record(
        conn,
        &Change {
            module: crate::MODULE_ID.to_owned(),
            entity: ENTITY.to_owned(),
            entity_id: device.to_owned(),
            op: if existed {
                ChangeOp::Update
            } else {
                ChangeOp::Create
            },
            session_id: None,
            snapshot: json!({ "name": name }),
        },
    )
}

/// This device was renamed: tells the group, if there is one.
pub fn record_own(conn: &Connection, name: &str) -> Result<(), SyncError> {
    if crate::peers::group(conn)?.is_none() {
        return Ok(());
    }
    let me = hlc::device_id(conn)?;
    Ok(record(conn, &me, name)?)
}

/// Renames the paired device `device` for the whole group. `my_name` is this device's own name,
/// which the new one must also differ from. Returns the name saved.
pub fn rename_peer(
    conn: &mut Connection,
    my_name: &str,
    device: &str,
    wanted: &str,
) -> Result<String, SyncError> {
    let name = validate_name(wanted)?;
    let tx = conn.transaction()?;
    let old: Option<String> = tx
        .query_row("SELECT name FROM sync_peers WHERE id = ?1", [device], |r| {
            r.get(0)
        })
        .optional()?;
    let Some(old) = old else {
        return Err(SyncError::Validation("That device is not paired"));
    };
    let mut others: Vec<String> = vec![my_name.to_owned()];
    {
        let mut stmt = tx.prepare("SELECT name FROM sync_peers WHERE id <> ?1")?;
        others.extend(
            stmt.query_map([device], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    if unique_name(&name, &others) != name {
        return Err(SyncError::Validation(
            "Another of your devices already has that name",
        ));
    }
    if name == old {
        return Ok(name);
    }
    tx.execute(
        "UPDATE sync_peers SET name = ?2 WHERE id = ?1",
        params![device, name],
    )?;
    record(&tx, device, &name)?;
    crate::activity::log_renamed(&tx, device, &old, &name)?;
    tx.commit()?;
    Ok(name)
}

/// Takes a rename received from another device. A name that would clash here gets a number, the
/// way a name does at pairing.
pub(crate) fn apply(conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
    let device = change.entity_id;
    let Some(name) = change
        .fields
        .get("name")
        .and_then(Value::as_str)
        .and_then(|n| validate_name(n).ok())
    else {
        return Ok(Applied {
            snapshot: Value::Object(change.fields.clone()),
            changed: false,
            events: Vec::new(),
        });
    };
    let me = hlc::device_id(conn)?;
    let mut events = Vec::new();
    if device == me {
        events.push(SyncEvent::DeviceRenamed { name: name.clone() });
    } else {
        let mut stmt = conn.prepare("SELECT name FROM sync_peers WHERE id <> ?1")?;
        let others: Vec<String> = stmt
            .query_map([device], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;
        let fitted = unique_name(&name, &others);
        conn.execute(
            "UPDATE sync_peers SET name = ?2 WHERE id = ?1",
            params![device, fitted],
        )?;
    }
    Ok(Applied {
        snapshot: json!({ "name": name }),
        changed: false,
        events,
    })
}
