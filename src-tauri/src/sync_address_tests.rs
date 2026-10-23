//! The Address typed for a device is shared by the whole group (a Value field of the synced
//! `sync_device` record): typed on one device it reaches the others, directly or through a
//! relay; the newest edit wins and is logged; clearing is an edit; a device ignores the one typed
//! for itself. Every step is a real command; every assertion reads back through commands
//! (ADR 0009). Dialling it is covered by `sync_forget_tests`.

use serde_json::{Value, json};

use crate::test_support::{Cluster, TestApp};

const NAMES: [&str; 4] = ["Mac", "Pixel 8", "Tablet", "Watch"];

/// `n` devices, each named and knowing the others as paired devices of one group.
fn group(n: usize) -> Cluster {
    let c = Cluster::new(n);
    let ids: Vec<String> = (0..n).map(|i| c[i].device_id()).collect();
    for i in 0..n {
        c[i].invoke::<()>("sync_rename", json!({ "name": NAMES[i] }))
            .unwrap();
        c[i].with_db(|db| {
            db.execute(
                "INSERT INTO sync_group(one, id) VALUES (1, 'group') ON CONFLICT DO NOTHING",
                [],
            )
            .unwrap();
            for j in (0..n).filter(|j| *j != i) {
                db.execute(
                    "INSERT INTO sync_peers(id, name, paired_at) VALUES (?1, ?2, 1)",
                    [ids[j].as_str(), NAMES[j]],
                )
                .unwrap();
            }
        });
    }
    c
}

fn type_address(app: &TestApp, device: &str, address: &str) -> Value {
    app.invoke(
        "sync_device_address",
        json!({ "id": device, "address": address }),
    )
    .unwrap()
}

/// The Address a device shows on the sheet of `device`.
fn shown(app: &TestApp, device: &str) -> Value {
    let sheet: Value = app.invoke("sync_device", json!({ "id": device })).unwrap();
    sheet["typed_address"].clone()
}

fn value_rows(app: &TestApp) -> Vec<Value> {
    let list: Value = app.invoke("sync_conflicts", json!({})).unwrap();
    list["resolved"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "value")
        .cloned()
        .collect()
}

#[test]
fn an_address_typed_on_one_device_reaches_the_others_directly_and_through_a_relay() {
    let c = group(4);
    let watch = c[3].device_id();
    assert_eq!(
        type_address(&c[0], &watch, " Watch.TAIL1234.ts.net:47311 "),
        json!("watch.tail1234.ts.net:47311")
    );
    // The Mac meets the Pixel 8, the Pixel 8 the Tablet: the Tablet never meets the Mac.
    c.sync(0, 1);
    c.sync(1, 2);
    for d in 0..3 {
        assert_eq!(
            shown(&c[d], &watch),
            json!("watch.tail1234.ts.net:47311"),
            "device {d}"
        );
    }
    // The device it is typed for takes nothing for itself.
    c.sync(2, 3);
    c[3].with_db(|db| {
        let kept: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM sync_peers WHERE typed_address IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kept, 0, "the Watch has no Address for anybody");
    });
    assert!(value_rows(&c[3]).is_empty());
}

#[test]
fn any_member_can_edit_it_and_clearing_syncs() {
    let c = group(3);
    let watch = c[2].device_id();
    type_address(&c[0], &watch, "10.0.0.7");
    c.sync_all();
    assert_eq!(shown(&c[1], &watch), json!("10.0.0.7"));
    // The Pixel 8 changes it, later.
    c[1].clock().advance(5_000);
    type_address(&c[1], &watch, "10.0.0.8:47311");
    c.sync_all();
    assert_eq!(shown(&c[0], &watch), json!("10.0.0.8:47311"));
    // And clears it.
    c[0].clock().advance(5_000);
    assert!(type_address(&c[0], &watch, "").is_null());
    c.sync_all();
    assert!(shown(&c[1], &watch).is_null());
    // Later edits after a change of mind are plain changes, not clashes.
    assert!(value_rows(&c[0]).is_empty() && value_rows(&c[1]).is_empty());
}

#[test]
fn concurrent_edits_converge_to_the_newest_and_are_logged_the_same_on_both() {
    let c = group(3);
    let watch = c[2].device_id();
    type_address(&c[0], &watch, "10.0.0.7");
    c.sync_all();
    // Both change it without having heard of the other; the Pixel 8 is the later.
    c[0].clock().advance(1_000);
    type_address(&c[0], &watch, "10.0.0.20");
    c[1].clock().advance(5_000);
    type_address(&c[1], &watch, "10.0.0.30");
    c.sync_all();
    for d in 0..2 {
        assert_eq!(shown(&c[d], &watch), json!("10.0.0.30"), "device {d}");
    }
    let (a, b) = (value_rows(&c[0]), value_rows(&c[1]));
    assert_eq!((a.len(), b.len()), (1, 1), "{a:?} {b:?}");
    assert_eq!(a[0]["id"], b[0]["id"]);
    assert_eq!(a[0]["title"], "Tablet · Address");
    assert_eq!(
        a[0]["detail"],
        "10.0.0.30 (Pixel 8) kept · 10.0.0.20 (Mac) replaced"
    );
    assert!(a[0]["action"].is_null(), "nothing to write back");
}

#[test]
fn an_address_typed_for_a_device_is_ignored_by_that_device() {
    let c = group(2);
    let (mac, pixel) = (c[0].device_id(), c[1].device_id());
    // Typed for the Pixel 8 on the Mac, and for the Mac on the Pixel 8.
    type_address(&c[0], &pixel, "pixel.example.net");
    c[1].clock().advance(1_000);
    type_address(&c[1], &mac, "mac.example.net");
    c.sync_all();
    assert_eq!(shown(&c[0], &pixel), json!("pixel.example.net"));
    assert_eq!(shown(&c[1], &mac), json!("mac.example.net"));
    // The Pixel 8 holds what was typed for the Mac, not what was typed for itself.
    for (d, own) in [(0usize, 0usize), (1, 1)] {
        c[d].with_db(|db| {
            let held: Vec<(String, String)> = db
                .prepare("SELECT id, typed_address FROM sync_peers WHERE typed_address IS NOT NULL")
                .unwrap()
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            assert_eq!(held.len(), 1, "device {d}: {held:?}");
            assert_ne!(held[0].0, c[own].device_id());
        });
    }
}

#[test]
fn an_address_typed_before_it_synced_joins_the_group_once() {
    let c = group(3);
    let watch = c[2].device_id();
    // As an earlier version left it: on the row, never in the log.
    c[0].with_db(|db| {
        db.execute(
            "UPDATE sync_peers SET typed_address = '100.64.0.9:47311' WHERE id = ?1",
            [&watch],
        )
        .unwrap();
    });
    c[0].restart();
    c.sync_all();
    assert_eq!(shown(&c[1], &watch), json!("100.64.0.9:47311"));
    // Starting again records nothing more, and the newer word of another device stands.
    let before = c[0].with_db(|db| {
        db.query_row(
            "SELECT COUNT(*) FROM change_log WHERE entity = 'sync_device'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
    });
    c[0].restart();
    let after = c[0].with_db(|db| {
        db.query_row(
            "SELECT COUNT(*) FROM change_log WHERE entity = 'sync_device'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
    });
    assert_eq!(before, after);
    c[1].clock().advance(5_000);
    type_address(&c[1], &watch, "10.1.1.1");
    c[0].restart();
    c.sync_all();
    assert_eq!(shown(&c[0], &watch), json!("10.1.1.1"));
}
