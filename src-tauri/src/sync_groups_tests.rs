//! Groups of three or more (#70): changes relay through any device, and pairing with one member
//! joins the whole group. Scenarios on a [`Cluster`] (in-memory transport) and between `TestApp`s
//! over real loopback sockets (ADR 0009).

use mnemora_sync::net::DeviceIdentity;
use serde_json::{Value, json};

use crate::sync_live_tests::{create, has, meet, quick, until};
use crate::sync_pairing_tests::Device;
use crate::test_support::Cluster;

/// `shower` shows the code and `enterer` types it. Returns what each side's success screen says.
async fn pair_views(shower: &Device, enterer: &Device) -> (Value, Value) {
    shower.call::<()>("sync_pair_show", json!({}));
    let code = shower.pairing()["code"].as_str().unwrap().replace(' ', "");
    enterer.found.set(vec![shower.address()]);
    enterer.call::<()>("sync_pair_enter", json!({ "code": code }));
    shower.until("asking").await;
    shower.call::<()>("sync_pair_decide", json!({ "allow": true }));
    (shower.until("paired").await, enterer.until("paired").await)
}

fn also_joined(view: &Value) -> Vec<String> {
    let mut names: Vec<String> = view["also_joined"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap().to_owned())
        .collect();
    names.sort();
    names
}

fn state_of(d: &Device, name: &str) -> Value {
    crate::sync_live_tests::row_named(d, name)["state"].clone()
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

fn make(app: &crate::test_support::TestApp, title: &str) -> Value {
    app.invoke("todo_task_create", json!({ "title": title }))
        .unwrap()
}

fn title_of(app: &crate::test_support::TestApp, id: &Value) -> String {
    app.invoke::<Value>("todo_task", json!({ "id": id }))
        .unwrap()["title"]
        .as_str()
        .unwrap()
        .to_owned()
}

// ---------------------------------------------------------------- relay (Cluster)

#[test]
fn a_change_from_a_reaches_c_when_a_meets_b_and_then_b_meets_c() {
    let c = Cluster::new(3);
    let made = make(&c[0], "From A");
    c.sync(0, 1);
    c.sync(1, 2);
    assert_eq!(title_of(&c[2], &made["id"]), "From A");
    // And back: C's edit reaches A without the two ever meeting.
    c[2].clock().advance(1_000);
    c[2].invoke::<Value>(
        "todo_task_set_priority",
        json!({ "id": made["id"], "priority": 2 }),
    )
    .unwrap();
    c.sync(2, 1);
    c.sync(1, 0);
    let on_a: Value = c[0]
        .invoke("todo_task", json!({ "id": made["id"] }))
        .unwrap();
    assert_eq!(on_a["priority"], 2);
}

#[test]
fn four_devices_meeting_in_a_chain_converge_on_everything_each_made() {
    let c = Cluster::new(4);
    let ids: Vec<Value> = (0..4)
        .map(|d| make(&c[d], &format!("Made on {d}"))["id"].clone())
        .collect();
    // A line: 0-1, 1-2, 2-3, then back, so each change has travelled the whole way.
    for (a, b) in [(0, 1), (1, 2), (2, 3), (2, 3), (1, 2), (0, 1)] {
        c.sync(a, b);
    }
    for d in 0..4 {
        for (made_on, id) in ids.iter().enumerate() {
            assert_eq!(
                title_of(&c[d], id),
                format!("Made on {made_on}"),
                "device {d}"
            );
        }
    }
    // Edits made concurrently on all four settle to one answer everywhere.
    for (d, p) in [(0usize, 1u32), (1, 2), (2, 3), (3, 1)] {
        c[d].clock().advance(100 * (u64_to_i64(d) + 1));
        c[d].invoke::<Value>(
            "todo_task_set_priority",
            json!({ "id": ids[0], "priority": p }),
        )
        .unwrap();
    }
    for (a, b) in [(0, 1), (1, 2), (2, 3), (2, 3), (1, 2), (0, 1)] {
        c.sync(a, b);
    }
    let first = c[0]
        .invoke::<Value>("todo_task", json!({ "id": ids[0] }))
        .unwrap()["priority"]
        .clone();
    assert_eq!(first, 1, "device 3 made the newest change");
    for d in 1..4 {
        let seen = c[d]
            .invoke::<Value>("todo_task", json!({ "id": ids[0] }))
            .unwrap()["priority"]
            .clone();
        assert_eq!(seen, first, "device {d}");
    }
}

fn u64_to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap()
}

#[test]
fn an_introduction_signed_by_a_device_outside_the_group_changes_nothing() {
    let c = Cluster::new(2);
    // Device 1 is nobody's pairing: it signs an introduction of "mallory" all the same.
    let outsider = DeviceIdentity::generate().unwrap();
    let mallory = DeviceIdentity::generate().unwrap();
    c[1].with_db(|db| {
        db.execute(
            "INSERT INTO sync_group(one, id) VALUES (1, 'g')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sync_peers(id, name, paired_at, public_key, platform) VALUES ('mallory', 'Mallory', 1, ?1, 'phone')",
            [mallory.public_key().to_hex()],
        )
        .unwrap();
        db.execute(
            "INSERT INTO sync_peers(id, name, paired_at, public_key, platform) VALUES ('erin', 'Erin', 1, ?1, 'phone')",
            [DeviceIdentity::generate().unwrap().public_key().to_hex()],
        )
        .unwrap();
        let made = mnemora_sync::introduce_members(db, &outsider, &["mallory".to_owned()]).unwrap();
        assert_eq!(made, 1);
    });
    c.sync(1, 0);
    let paired: i64 = c[0].with_db(|db| {
        db.query_row("SELECT COUNT(*) FROM sync_peers", [], |r| r.get(0))
            .unwrap()
    });
    assert_eq!(paired, 0, "a record signed by a stranger pairs nobody");
    // It is still passed on, unchanged, to whoever else could judge it.
    let kept: i64 = c[0].with_db(|db| {
        db.query_row(
            "SELECT COUNT(*) FROM change_log WHERE entity = 'sync_introduction'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    });
    assert_eq!(kept, 1);
}

// ---------------------------------------------------------------- joining the group (loopback)

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_newcomer_paired_via_one_member_is_accepted_by_the_others_and_connects_directly() {
    let a = Device::with_pace("Alice", quick()).await;
    let b = Device::with_pace("Bob", quick()).await;
    let c = Device::with_pace("Carol", quick()).await;
    let d = Device::with_pace("Dana", quick()).await;
    pair_views(&a, &b).await;
    // Carol joins through Bob; her success screen names Alice, and Bob's names nobody new.
    let (bob_view, carol_view) = pair_views(&b, &c).await;
    assert_eq!(also_joined(&carol_view), ["Alice"]);
    assert_eq!(also_joined(&bob_view), Vec::<String>::new());
    // Dana joins through Bob too and hears of both.
    let (_, dana_view) = pair_views(&b, &d).await;
    assert_eq!(also_joined(&dana_view), ["Alice", "Carol"]);
    assert_eq!(sorted(d.listed()), ["Alice", "Bob", "Carol"]);

    // Alice and Carol have only met Bob so far.
    assert_eq!(a.listed(), ["Bob"]);
    assert_eq!(sorted(c.listed()), ["Alice", "Bob"]);

    // Bob meets Alice and Carol: his introductions travel with the ordinary sync.
    meet(&a, &b);
    meet(&b, &c);
    until("Alice and Carol to list everybody", || {
        sorted(a.listed()) == ["Bob", "Carol", "Dana"]
            && sorted(c.listed()) == ["Alice", "Bob", "Dana"]
    })
    .await;

    // Now Dana reaches Alice and Carol on her own, with no help from Bob.
    meet(&a, &d);
    meet(&c, &d);
    until("Dana connected to Alice and Carol directly", || {
        state_of(&a, "Dana") == "up_to_date"
            && state_of(&d, "Alice") == "up_to_date"
            && state_of(&c, "Dana") == "up_to_date"
            && state_of(&d, "Carol") == "up_to_date"
    })
    .await;
    let made = create(&d, "Dana's own");
    until("the Task on Alice and Carol", || {
        has(&a, &made) && has(&c, &made)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_forgotten_device_paired_again_via_a_member_that_had_not_heard_comes_back_everywhere() {
    let a = Device::with_pace("Alice", quick()).await;
    let b = Device::with_pace("Bob", quick()).await;
    let c = Device::with_pace("Carol", quick()).await;
    pair_views(&a, &b).await;
    pair_views(&a, &c).await;
    meet(&a, &b);
    meet(&a, &c);
    meet(&b, &c);
    until("everyone knows everyone", || {
        b.listed().len() == 2 && c.listed().len() == 2
    })
    .await;
    // Alice forgets Carol; Bob hears. Carol pairs with Bob again, while Alice is away: Bob lifts
    // the Forget and introduces her, and Alice accepts both decisions in either order.
    a.call::<()>("sync_device_forget", json!({ "id": c.app.device_id() }));
    until("Bob to drop Carol", || b.listed() == ["Alice"]).await;
    pair_views(&b, &c).await;
    until("Alice to take Carol back", || {
        sorted(a.listed()) == ["Bob", "Carol"]
    })
    .await;
}
