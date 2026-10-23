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

const TAKEN: &str = "Another of your devices already has that name";

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
        return Err(SyncError::Validation(TAKEN));
    }
    if name == old {
        return Ok(name);
    }
    settle(&tx, device, &name)?;
    record(&tx, device, &name)?;
    crate::activity::log_renamed(&tx, device, &old, &name)?;
    tx.commit()?;
    Ok(name)
}

/// The name this device may take: `wanted`, trimmed, when no paired device already has it.
pub fn check_own_name(conn: &Connection, wanted: &str) -> Result<String, SyncError> {
    let name = validate_name(wanted)?;
    let mut stmt = conn.prepare("SELECT name FROM sync_peers")?;
    let others = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if unique_name(&name, &others) != name {
        return Err(SyncError::Validation(TAKEN));
    }
    Ok(name)
}

/// Whether two names are the same name to a person (case, accents and spacing ignored).
fn same_name(a: &str, b: &str) -> bool {
    unique_name(a, &[b.to_owned()]) != a
}

/// Records what `device` asked to be called and shows every device that asked for the same name
/// under it, or under it with a number. The lowest id keeps the plain name, the next gets "(2)",
/// and so on: a function of the wishes alone, so devices that heard two renames in a different
/// order still agree.
pub(crate) fn settle(conn: &Connection, device: &str, wanted: &str) -> AppResult<()> {
    let before: Option<String> = conn
        .query_row(
            "SELECT COALESCE(wanted_name, name) FROM sync_peers WHERE id = ?1",
            [device],
            |r| r.get(0),
        )
        .optional()?;
    conn.execute(
        "UPDATE sync_peers SET wanted_name = ?2 WHERE id = ?1",
        params![device, wanted],
    )?;
    settle_family(conn, wanted)?;
    // The name this one left behind may be free for another to take.
    match before.filter(|b| !same_name(b, wanted)) {
        Some(before) => settle_family(conn, &before),
        None => Ok(()),
    }
}

/// Names the devices that want `name`, lowest id first.
fn settle_family(conn: &Connection, name: &str) -> AppResult<()> {
    let mut stmt =
        conn.prepare("SELECT id, name, COALESCE(wanted_name, name) FROM sync_peers ORDER BY id")?;
    let all: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let (family, rest): (Vec<_>, Vec<_>) =
        all.into_iter().partition(|(_, _, w)| same_name(w, name));
    let mut taken: Vec<String> = rest.into_iter().map(|(_, n, _)| n).collect();
    for (id, shown, wanted) in family {
        let fitted = unique_name(&wanted, &taken);
        if fitted != shown {
            conn.execute(
                "UPDATE sync_peers SET name = ?2 WHERE id = ?1",
                params![id, fitted],
            )?;
        }
        taken.push(fitted);
    }
    Ok(())
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
        settle(conn, device, &name)?;
    }
    Ok(Applied {
        snapshot: json!({ "name": name }),
        changed: false,
        events,
    })
}

#[cfg(test)]
mod tests {
    use mnemora_core::db;
    use mnemora_core::settings::SettingsRegistry;

    use super::*;

    fn conn(peers: &[(&str, &str)]) -> Connection {
        let mut conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        db::migrate::run(
            &mut conn,
            &[&mnemora_core::module::CoreModule, &crate::SyncModule],
            dir.path(),
        )
        .unwrap();
        hlc::set_device_id(&conn, "me").unwrap();
        conn.execute("INSERT INTO sync_group(one, id) VALUES (1, 'g')", [])
            .unwrap();
        for (id, name) in peers {
            conn.execute(
                "INSERT INTO sync_peers(id, name, paired_at, platform) VALUES (?1, ?2, 1, 'phone')",
                [id, name],
            )
            .unwrap();
        }
        conn
    }

    fn receive(conn: &Connection, device: &str, name: &str) {
        let fields = json!({ "name": name });
        let registry = SettingsRegistry::default();
        let change = RemoteChange {
            entity: ENTITY,
            entity_id: device,
            op: ChangeOp::Update,
            at: 1,
            fields: fields.as_object().unwrap(),
            settings: &registry,
        };
        apply(conn, &change).unwrap();
    }

    fn names(conn: &Connection) -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare("SELECT id, name FROM sync_peers ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn two_renames_to_one_name_settle_the_same_whatever_order_they_arrive_in() {
        let peers = [("x", "Pixel"), ("y", "Tab"), ("z", "Watch")];
        let first = conn(&peers);
        receive(&first, "y", "Phone");
        receive(&first, "x", "Phone");
        let second = conn(&peers);
        receive(&second, "x", "Phone");
        receive(&second, "y", "Phone");
        assert_eq!(names(&first), names(&second));
        assert_eq!(
            names(&first),
            [
                ("x".to_owned(), "Phone".to_owned()),
                ("y".to_owned(), "Phone (2)".to_owned()),
                ("z".to_owned(), "Watch".to_owned()),
            ],
            "the lowest id keeps the plain name"
        );
    }

    #[test]
    fn three_renames_to_one_name_settle_the_same_in_every_order() {
        let peers = [("a", "A"), ("b", "B"), ("c", "C")];
        let orders = [
            ["a", "b", "c"],
            ["c", "b", "a"],
            ["b", "c", "a"],
            ["b", "a", "c"],
        ];
        let settled: Vec<_> = orders
            .iter()
            .map(|order| {
                let conn = conn(&peers);
                for id in order {
                    receive(&conn, id, "Phone");
                }
                names(&conn)
            })
            .collect();
        assert!(settled.windows(2).all(|w| w[0] == w[1]), "{settled:?}");
        let got: Vec<&str> = settled[0].iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(got, ["Phone", "Phone (2)", "Phone (3)"]);
    }

    #[test]
    fn a_rename_to_a_free_name_changes_only_that_device() {
        let conn = conn(&[("x", "Pixel"), ("y", "Tab")]);
        receive(&conn, "y", "Tablet");
        assert_eq!(
            names(&conn),
            [
                ("x".to_owned(), "Pixel".to_owned()),
                ("y".to_owned(), "Tablet".to_owned())
            ]
        );
    }

    #[test]
    fn the_plain_name_goes_back_to_the_device_left_with_the_number() {
        let conn = conn(&[("x", "Pixel"), ("y", "Tab")]);
        receive(&conn, "x", "Phone");
        receive(&conn, "y", "Phone");
        receive(&conn, "x", "Tablet");
        assert_eq!(
            names(&conn),
            [
                ("x".to_owned(), "Tablet".to_owned()),
                ("y".to_owned(), "Phone".to_owned())
            ]
        );
    }
}
