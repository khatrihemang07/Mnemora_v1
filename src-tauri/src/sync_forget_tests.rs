//! The device sheet, Forget and Leave between `TestApp`s over real loopback sockets: the commands
//! the UI calls and what each device shows afterwards (ADR 0009). Three devices are paired with
//! each other so a forgotten one has more than one member to be refused by.

use std::time::Duration;

use serde_json::{Value, json};

use crate::sync_live_tests::{create, has, meet, peer_of, quick, row_named, task_count, until};
use crate::sync_pairing_tests::Device;

/// Alice, Bob and Carol, each paired with the other two.
async fn trio() -> (Device, Device, Device) {
    let a = Device::with_pace("Alice", quick()).await;
    let b = Device::with_pace("Bob", quick()).await;
    let c = Device::with_pace("Carol", quick()).await;
    crate::sync_live_tests::pair(&a, &b).await;
    crate::sync_live_tests::pair(&a, &c).await;
    // Carol already knows Bob (Alice introduced him); Bob shows the code, Carol types it.
    crate::sync_live_tests::pair(&b, &c).await;
    (a, b, c)
}

fn meet_all(a: &Device, b: &Device, c: &Device) {
    meet(a, b);
    meet(a, c);
    meet(b, c);
}

fn id(d: &Device) -> String {
    d.app.device_id()
}

fn state_of(d: &Device, name: &str) -> Value {
    row_named(d, name)["state"].clone()
}

/// Carol cannot reach the member: it is "Not nearby", or "Removed" when a member that had not yet
/// dropped her passed the Forget on first (which of the two is a race).
fn cut_off(d: &Device, name: &str) -> bool {
    let state = state_of(d, name);
    state == "not_nearby" || state == "removed"
}

fn all_up_to_date(d: &Device) -> bool {
    let peers = d.devices()["peers"].as_array().unwrap().clone();
    peers.len() == 2 && peers.iter().all(|p| p["state"] == "up_to_date")
}

fn check(d: &Device, device: &str, address: &str) -> Value {
    d.call(
        "sync_device_check",
        json!({ "id": device, "address": address }),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_forgotten_device_is_refused_by_every_member() {
    let (a, b, c) = trio().await;
    meet_all(&a, &b, &c);
    until("everyone connected", || {
        all_up_to_date(&a) && all_up_to_date(&b) && all_up_to_date(&c)
    })
    .await;

    a.call::<()>("sync_device_forget", json!({ "id": id(&c) }));
    assert_eq!(a.listed(), ["Bob"], "gone from the device that forgot it");
    // Bob learns it from Alice, through an ordinary sync.
    until("Bob to drop Carol", || b.listed() == ["Alice"]).await;

    // Carol is refused by both: it keeps trying and never gets in.
    c.found.announce(&id(&a), a.address());
    c.found.announce(&id(&b), b.address());
    until("Carol to lose both", || {
        cut_off(&c, "Alice") && cut_off(&c, "Bob")
    })
    .await;
    let secret = create(&c, "Carol's own");
    let shared = create(&a, "For the others");
    until("Bob to get Alice's Task", || has(&b, &shared)).await;
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert!(!has(&a, &secret) && !has(&b, &secret), "Carol still synced");
    assert!(!has(&c, &shared), "Carol still received");
    assert!(cut_off(&c, "Alice") && cut_off(&c, "Bob"));

    // The same thing, asked of the network directly: neither member answers Carol.
    for member in [&a, &b] {
        let was = check(&c, &id(member), &member.address().to_string());
        assert_eq!(was["kind"], "not_paired", "{was}");
    }
    // Its data stays on it.
    assert!(has(&c, &secret));
    // Alice wrote it in Activity.
    let forgot: Value = a.call(
        "sync_activity",
        json!({ "filter": { "kind": "all" }, "cursor": null, "limit": 100 }),
    );
    assert!(
        forgot["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "forgot"),
        "{forgot}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pairing_a_forgotten_device_again_takes_it_back_everywhere() {
    let (a, b, c) = trio().await;
    meet_all(&a, &b, &c);
    until("everyone connected", || {
        all_up_to_date(&a) && all_up_to_date(&b) && all_up_to_date(&c)
    })
    .await;
    a.call::<()>("sync_device_forget", json!({ "id": id(&c) }));
    until("Bob to drop Carol", || b.listed() == ["Alice"]).await;
    until("Carol to lose both", || {
        cut_off(&c, "Alice") && cut_off(&c, "Bob")
    })
    .await;

    crate::sync_live_tests::pair(&a, &c).await;
    assert!(a.listed().contains(&"Carol".to_owned()));
    // Bob takes Carol back too, without being paired with it again.
    until("Bob to list Carol", || {
        b.listed().contains(&"Carol".to_owned())
    })
    .await;
    meet_all(&a, &b, &c);
    until("everyone connected again", || {
        all_up_to_date(&a) && all_up_to_date(&b) && all_up_to_date(&c)
    })
    .await;
    let made = create(&c, "Back in step");
    until("Task on both", || has(&a, &made) && has(&b, &made)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn leaving_stops_syncing_for_good_and_keeps_the_data() {
    let (a, b) = crate::sync_live_tests::paired_pair(quick()).await;
    meet(&a, &b);
    until("connected", || peer_of(&a)["state"] == "up_to_date").await;
    let before = create(&a, "Written before leaving");
    until("Bob has it", || has(&b, &before)).await;

    a.call::<()>("sync_leave", json!({}));
    assert!(a.listed().is_empty());
    assert_eq!(a.call::<Value>("sync_get", json!({}))["paired_devices"], 0);
    let group: i64 = a.app.with_db(|c| {
        c.query_row("SELECT COUNT(*) FROM sync_group", [], |r| r.get(0))
            .unwrap()
    });
    assert_eq!(group, 0, "its group id is gone");
    assert!(has(&a, &before), "the data stays");

    // Nothing travels either way, now or later, however hard both try.
    let after = create(&a, "Written after leaving");
    let theirs = create(&b, "Bob keeps writing");
    b.call::<()>("sync_now", json!({}));
    a.call::<()>("sync_now", json!({}));
    meet(&a, &b);
    tokio::time::sleep(Duration::from_millis(1_800)).await;
    assert!(!has(&b, &after) && !has(&a, &theirs));
    assert_eq!(peer_of(&b)["state"], "not_nearby");
    assert_eq!(task_count(&a), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_says_who_answered_in_words() {
    let (a, b, c) = trio().await;
    let (a_id, b_id, c_id) = (id(&a), id(&b), id(&c));

    let ok = check(&a, &b_id, &b.address().to_string());
    assert_eq!(ok["reached"], true, "{ok}");
    assert_eq!(ok["kind"], "reached");
    let message = ok["message"].as_str().unwrap();
    assert!(
        message.starts_with("Reached Bob in ") && message.ends_with(" ms"),
        "{message}"
    );
    assert!(ok["ms"].is_u64());

    // A name that resolves, with the port.
    let port = b.address().port();
    let by_name = check(&a, &b_id, &format!("localhost:{port}"));
    assert_eq!(by_name["reached"], true, "{by_name}");

    // Nothing is listening.
    let closed = {
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        socket.local_addr().unwrap()
    };
    let nobody = check(&a, &b_id, &closed.to_string());
    assert_eq!(nobody["kind"], "unreachable", "{nobody}");
    assert_eq!(nobody["reached"], false);

    // Another of the devices is there.
    let wrong = check(&a, &c_id, &b.address().to_string());
    assert_eq!(wrong["kind"], "wrong_device", "{wrong}");
    assert_eq!(wrong["message"], "That is Bob, not Carol.");

    // Not an address; a name that is nowhere.
    let bad = check(&a, &b_id, "not an address!");
    assert_eq!(bad["kind"], "invalid");
    assert_eq!(bad["message"], "That isn't a host name or an IP address");
    let nowhere = check(&a, &b_id, "no-such-device.invalid");
    assert_eq!(nowhere["kind"], "not_found", "{nowhere}");

    // A device that is not paired.
    let err = a
        .app
        .invoke::<Value>(
            "sync_device_check",
            json!({ "id": "nobody", "address": "127.0.0.1" }),
        )
        .unwrap_err();
    assert_eq!(err["code"], "not_found");
    // The answer is remembered as an address that worked.
    let sheet: Value = a.call("sync_device", json!({ "id": b_id }));
    let worked: Vec<String> = sheet["addresses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["address"].as_str().unwrap().to_owned())
        .collect();
    assert!(worked.contains(&b.address().to_string()), "{sheet}");
    let _ = a_id;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_typed_address_is_a_way_to_reach_the_device() {
    let (a, b) = crate::sync_live_tests::paired_pair(quick()).await;
    // Nothing announced, nothing remembered: only the typed Address can lead there.
    a.app.with_db(|c| {
        c.execute("DELETE FROM sync_peer_addresses", []).unwrap();
    });
    let b_id = id(&b);
    let bad = a
        .app
        .invoke::<Value>(
            "sync_device_address",
            json!({ "id": b_id, "address": "pixel 8" }),
        )
        .unwrap_err();
    assert_eq!(bad["message"], "That isn't a host name or an IP address");
    let stored: Value = a.call(
        "sync_device_address",
        json!({ "id": b_id, "address": format!("  LOCALHOST:{} ", b.address().port()) }),
    );
    assert_eq!(stored, json!(format!("localhost:{}", b.address().port())));
    let sheet: Value = a.call("sync_device", json!({ "id": b_id }));
    assert_eq!(sheet["typed_address"], stored);
    until("reached through the typed Address", || {
        peer_of(&a)["state"] == "up_to_date"
    })
    .await;
    // Clearing it again.
    let cleared: Value = a.call("sync_device_address", json!({ "id": b_id, "address": "" }));
    assert!(cleared.is_null());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_sheet_shows_what_is_known_and_syncs_only_that_device() {
    let (a, b, c) = trio().await;
    // Alice only syncs when asked; Carol takes nothing it did not ask for, so Bob's own
    // relaying cannot reach it either.
    for device in [&a, &c] {
        device.call::<()>(
            "sync_set_option",
            json!({ "option": "automatic", "enabled": false }),
        );
    }
    meet_all(&a, &b, &c);
    let made = create(&a, "Only for Bob");
    a.call::<()>("sync_device_sync_now", json!({ "id": id(&b) }));
    until("Bob to get the Task", || has(&b, &made)).await;
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert!(!has(&c, &made), "Carol was not asked");

    let sheet: Value = a.call("sync_device", json!({ "id": id(&b) }));
    assert_eq!(sheet["name"], "Bob");
    assert_eq!(
        sheet["paired_by"], "Alice",
        "Alice showed the code: {sheet}"
    );
    assert!(sheet["app_version"].as_str().is_some_and(|v| !v.is_empty()));
    let modules: Vec<&str> = sheet["data_versions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["module"].as_str().unwrap())
        .collect();
    assert!(
        modules.contains(&"core") && modules.contains(&"todo"),
        "{sheet}"
    );
    assert!(sheet["paired_at"].as_u64().unwrap() > 0);
    let gone = a
        .app
        .invoke::<Value>("sync_device", json!({ "id": "nobody" }))
        .unwrap_err();
    assert_eq!(gone["code"], "not_found");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rename_is_unique_and_reaches_every_device() {
    let (a, b, c) = trio().await;
    let c_id = id(&c);
    // Not a name another device has, whatever the case or accents.
    for clash in ["bob", "ALICE", " Bób "] {
        let err = a
            .app
            .invoke::<String>("sync_device_rename", json!({ "id": c_id, "name": clash }))
            .unwrap_err();
        assert_eq!(err["code"], "validation", "{clash}");
    }
    for bad in ["", &"x".repeat(41)] {
        assert!(
            a.app
                .invoke::<String>("sync_device_rename", json!({ "id": c_id, "name": bad }))
                .is_err()
        );
    }
    let saved: String = a.call(
        "sync_device_rename",
        json!({ "id": c_id, "name": "  Charlie " }),
    );
    assert_eq!(saved, "Charlie");
    assert_eq!(row_named(&a, "Charlie")["id"], json!(c_id));
    meet_all(&a, &b, &c);
    until("Bob and Carol to take the name", || {
        row_named(&b, "Charlie")["id"] == json!(c_id)
            && c.call::<Value>("sync_get", json!({}))["name"] == "Charlie"
    })
    .await;
    // The rename is in Activity.
    let page: Value = a.call(
        "sync_activity",
        json!({ "filter": { "kind": "all" }, "cursor": null, "limit": 100 }),
    );
    let renamed = page["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "renamed")
        .count();
    assert_eq!(renamed, 1, "{page}");
    // A device renaming itself in Settings reaches the others too.
    b.call::<()>("sync_rename", json!({ "name": "Robert" }));
    until("Alice to see Robert", || {
        row_named(&a, "Robert")["id"] == json!(id(&b))
    })
    .await;
}
