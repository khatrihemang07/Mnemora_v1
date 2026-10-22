//! Live syncing between two `TestApp`s over real loopback sockets: the commands the UI calls and
//! what each device shows afterwards (ADR 0009). Discovery is simulated by telling each device
//! where the other announced itself.

use std::time::{Duration, Instant};

use mnemora_sync::discovery::Discovery;
use mnemora_sync::{Pace, RuntimeProbe, Schedule};
use serde_json::{Value, json};

use crate::sync_pairing_tests::Device;

/// Quick enough for a test, with the same shape as the app's pace.
fn quick() -> Pace {
    Pace {
        debounce: Duration::from_millis(400),
        dial_each: Duration::from_secs(3),
        retry: Schedule {
            steps: vec![Duration::from_millis(200)],
            steady: Duration::from_millis(200),
        },
    }
}

async fn pair(mac: &Device, phone: &Device) {
    mac.call::<()>("sync_pair_show", json!({}));
    let code = mac.pairing()["code"].as_str().unwrap().replace(' ', "");
    phone.found.set(vec![mac.address()]);
    phone.call::<()>("sync_pair_enter", json!({ "code": code }));
    mac.until("asking").await;
    mac.call::<()>("sync_pair_decide", json!({ "allow": true }));
    mac.until("paired").await;
    phone.until("paired").await;
}

/// Two paired devices that can find each other (as mDNS would let them).
async fn paired_pair(mac_pace: Pace) -> (Device, Device) {
    let mac = Device::with_pace("Mac", mac_pace).await;
    let phone = Device::with_pace("Pixel 8", quick()).await;
    pair(&mac, &phone).await;
    (mac, phone)
}

fn meet(mac: &Device, phone: &Device) {
    mac.found.announce(&phone.app.device_id(), phone.address());
    phone.found.announce(&mac.app.device_id(), mac.address());
}

fn peer_of(device: &Device) -> Value {
    device.devices()["peers"][0].clone()
}

async fn until(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("never saw {what}");
}

fn create(device: &Device, title: &str) -> Value {
    device
        .app
        .invoke::<Value>("todo_task_create", json!({ "title": title }))
        .unwrap()["id"]
        .clone()
}

fn has(device: &Device, id: &Value) -> bool {
    device
        .app
        .invoke::<Value>("todo_task", json!({ "id": id }))
        .is_ok()
}

fn task_count(device: &Device) -> i64 {
    device.app.with_db(|c| {
        c.query_row("SELECT COUNT(*) FROM todo_tasks", [], |r| r.get(0))
            .unwrap()
    })
}

#[tokio::test]
async fn paired_devices_that_find_each_other_connect_and_say_up_to_date() {
    let (mac, phone) = paired_pair(quick()).await;
    assert_eq!(
        peer_of(&mac)["state"],
        "not_nearby",
        "nobody has been found yet"
    );
    meet(&mac, &phone);
    until("both up to date", || {
        peer_of(&mac)["state"] == "up_to_date" && peer_of(&phone)["state"] == "up_to_date"
    })
    .await;
    assert_eq!(peer_of(&mac)["route"], "wifi");
    assert!(peer_of(&mac)["last_synced_at"].is_u64());
}

#[tokio::test]
async fn a_change_reaches_the_other_device_only_after_the_quiet_period() {
    // A longer quiet period than the others, so a slow machine cannot make the second change
    // miss it.
    let slow = Pace {
        debounce: Duration::from_secs(1),
        ..quick()
    };
    let (mac, phone) = paired_pair(slow).await;
    meet(&mac, &phone);
    until("connected", || peer_of(&mac)["state"] == "up_to_date").await;
    // Let the connection's first (empty) session finish before the change.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let signals = phone.app.widget_signals();
    let made = Instant::now();
    let id = create(&mac, "Plan weekend hike");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!has(&phone, &id), "pushed before the quiet period was over");
    // More changes keep the quiet period running.
    tokio::time::sleep(Duration::from_millis(300)).await;
    create(&mac, "And buy boots");
    until("the Task on the phone", || has(&phone, &id)).await;
    assert!(
        made.elapsed() >= Duration::from_millis(1_500),
        "the second change restarted the wait: {:?}",
        made.elapsed()
    );
    until("both Tasks", || task_count(&phone) == 2).await;
    // The widget is told when the session ends, a moment after the last batch is stored.
    until("the widget to be told", || {
        phone.app.widget_signals() > signals
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        phone.app.widget_signals(),
        signals + 1,
        "the Todo widget is told once for the session (was {signals})"
    );
    let row = peer_of(&mac);
    assert_eq!(row["last_out"], 2, "{row}");
    until("the phone's count", || peer_of(&phone)["last_in"] == 2).await;
}

#[tokio::test]
async fn a_big_sync_in_many_batches_refreshes_the_widget_once() {
    let (mac, phone) = paired_pair(quick()).await;
    for n in 0..1_100 {
        create(&mac, &format!("Task {n}"));
    }
    let signals = phone.app.widget_signals();
    meet(&mac, &phone);
    until("all Tasks", || task_count(&phone) == 1_100).await;
    until("the session to end", || {
        peer_of(&phone)["state"] == "up_to_date" && peer_of(&phone)["last_in"] == 1_100
    })
    .await;
    assert_eq!(
        phone.app.widget_signals(),
        signals + 1,
        "three batches, one refresh"
    );
}

#[tokio::test]
async fn with_sync_automatically_off_only_sync_now_syncs() {
    let (mac, phone) = paired_pair(quick()).await;
    mac.call::<()>(
        "sync_set_option",
        json!({ "option": "automatic", "enabled": false }),
    );
    meet(&mac, &phone);
    let id = create(&mac, "Only on request");
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert!(!has(&phone, &id), "synced by itself");
    assert_eq!(peer_of(&mac)["state"], "not_nearby");

    mac.call::<()>("sync_now", json!({}));
    until("the Task on the phone", || has(&phone, &id)).await;
    until("the row to settle", || {
        peer_of(&mac)["state"] == "up_to_date"
    })
    .await;
}

#[tokio::test]
async fn turning_sync_off_stops_every_connection_and_the_port() {
    let (mac, phone) = paired_pair(quick()).await;
    meet(&mac, &phone);
    until("connected", || peer_of(&phone)["state"] == "up_to_date").await;
    let addr = mac.address();
    assert!(mac.app.sync_probe().sessions >= 1);

    mac.call::<()>("sync_set_enabled", json!({ "enabled": false }));
    assert_eq!(
        mac.app.sync_probe(),
        RuntimeProbe {
            running: false,
            listeners: 0,
            advertisements: 0,
            tasks: 0,
            sessions: 0
        },
        "no listener, discovery, connection, dial or timer"
    );
    // The tasks are aborted, which finishes a moment later.
    until("announcing and browsing to stop", || {
        mac.found.active() == 0
    })
    .await;
    until("the port to close", || {
        std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_err()
    })
    .await;
    until("the phone to notice", || {
        peer_of(&phone)["state"] != "up_to_date"
    })
    .await;
    // The phone keeps trying; the Mac is not there to answer.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(peer_of(&phone)["state"], "not_nearby");
}

#[tokio::test]
async fn a_device_that_comes_back_is_reconnected_by_the_backoff() {
    let (mac, phone) = paired_pair(quick()).await;
    meet(&mac, &phone);
    until("connected", || peer_of(&phone)["state"] == "up_to_date").await;
    // The Mac stops syncing and starts again: its port may be new, the phone redials and learns it.
    mac.call::<()>("sync_set_enabled", json!({ "enabled": false }));
    until("the phone to notice", || {
        peer_of(&phone)["state"] == "not_nearby"
    })
    .await;
    mac.call::<()>("sync_set_enabled", json!({ "enabled": true }));
    // Bring the network up again by hand: the test network does not restart on its own.
    crate::sync::start_network(
        &mac.app.handle(),
        mnemora_sync::NetworkOptions {
            ip: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            port: 0,
            timing: mnemora_sync::Timing::default(),
            discovery: mac.found.clone(),
            pace: quick(),
        },
    )
    .await
    .unwrap();
    phone.found.announce(&mac.app.device_id(), mac.address());
    until("reconnected", || peer_of(&phone)["state"] == "up_to_date").await;
}

#[tokio::test]
async fn leaving_the_group_closes_the_connections() {
    let (mac, phone) = paired_pair(quick()).await;
    meet(&mac, &phone);
    until("connected", || peer_of(&phone)["state"] == "up_to_date").await;
    mac.call::<()>("sync_leave", json!({}));
    assert!(mac.devices()["peers"].as_array().unwrap().is_empty());
    until("the phone to lose it", || {
        peer_of(&phone)["state"] == "not_nearby"
    })
    .await;
    // The Mac no longer trusts the phone's key, so the phone's retries are refused.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(peer_of(&phone)["state"], "not_nearby");
}
