//! Live syncing between two `TestApp`s over real loopback sockets: the commands the UI calls and
//! what each device shows afterwards (ADR 0009). Discovery is simulated by telling each device
//! where the other announced itself.

use std::time::{Duration, Instant};

use mnemora_sync::discovery::Discovery;
use mnemora_sync::{Pace, RuntimeProbe, Schedule};
use serde_json::{Value, json};

use crate::sync_pairing_tests::Device;

/// Quick enough for a test, with the same shape as the app's pace.
pub(crate) fn quick() -> Pace {
    Pace {
        debounce: Duration::from_millis(400),
        dial_each: Duration::from_secs(3),
        retry: Schedule {
            steps: vec![Duration::from_millis(200)],
            steady: Duration::from_millis(200),
        },
    }
}

pub(crate) async fn pair(mac: &Device, phone: &Device) {
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
pub(crate) async fn paired_pair(mac_pace: Pace) -> (Device, Device) {
    let mac = Device::with_pace("Mac", mac_pace).await;
    let phone = Device::with_pace("Pixel 8", quick()).await;
    pair(&mac, &phone).await;
    (mac, phone)
}

pub(crate) fn meet(mac: &Device, phone: &Device) {
    mac.found.announce(&phone.app.device_id(), phone.address());
    phone.found.announce(&mac.app.device_id(), mac.address());
}

pub(crate) fn peer_of(device: &Device) -> Value {
    device.devices()["peers"][0].clone()
}

pub(crate) async fn until(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    // Generous: the whole suite runs these in parallel on a small machine.
    while start.elapsed() < Duration::from_secs(20) {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("never saw {what}");
}

pub(crate) fn create(device: &Device, title: &str) -> Value {
    device
        .app
        .invoke::<Value>("todo_task_create", json!({ "title": title }))
        .unwrap()["id"]
        .clone()
}

pub(crate) fn has(device: &Device, id: &Value) -> bool {
    device
        .app
        .invoke::<Value>("todo_task", json!({ "id": id }))
        .is_ok()
}

pub(crate) fn task_count(device: &Device) -> i64 {
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
    // The first session may still be finishing (pairing left a line in Activity to pass on).
    until("the first session to finish", || {
        peer_of(&mac)["last_synced_at"].is_u64()
    })
    .await;
    assert_eq!(peer_of(&mac)["route"], "wifi");
}

/// Both devices dial at once and one of the two connections is torn down as the duplicate: both
/// rows must settle on "Up to date" and stay there (DV2), never "Not nearby" on either.
#[tokio::test]
async fn a_simultaneous_dial_and_its_duplicate_teardown_leave_both_rows_up_to_date() {
    let (mac, phone) = paired_pair(quick()).await;
    meet(&mac, &phone);
    until("both up to date", || {
        peer_of(&mac)["state"] == "up_to_date" && peer_of(&phone)["state"] == "up_to_date"
    })
    .await;
    // The first session's own follow-up round (the quiet period, then a short session that reads
    // `syncing`) must be over before the rows are held to `up_to_date`; what is checked below is
    // that the duplicate's teardown leaves nothing wrong once the sessions have settled.
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    until("both settled", || {
        peer_of(&mac)["state"] == "up_to_date" && peer_of(&phone)["state"] == "up_to_date"
    })
    .await;
    // Long enough for every racing dial to finish or give up.
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(peer_of(&mac)["state"], "up_to_date", "Mac's row");
        assert_eq!(peer_of(&phone)["state"], "up_to_date", "Pixel's row");
    }
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
    // Let the connection's first session finish, and the quiet period that pairing's line in
    // Activity started run out, before the change.
    tokio::time::sleep(Duration::from_millis(1_500)).await;

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
    until("connected", || {
        peer_of(&phone)["state"] == "up_to_date" && mac.app.sync_probe().sessions >= 1
    })
    .await;
    let addr = mac.address();

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
            ip6: None,
            timing: mnemora_sync::Timing::default(),
            discovery: mac.found.clone(),
            pace: quick(),
            local_network: mnemora_sync::local_network::LocalNetwork::off(),
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

pub(crate) fn row_named(device: &Device, name: &str) -> Value {
    device.devices()["peers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .cloned()
        .unwrap_or(Value::Null)
}

#[tokio::test]
async fn what_a_device_received_is_passed_on_to_the_others_it_is_connected_to() {
    // Mac and Watch cannot reach each other; both reach the phone.
    let (mac, phone) = paired_pair(quick()).await;
    let watch = Device::with_pace("Watch", quick()).await;
    pair(&phone, &watch).await;
    meet(&mac, &phone);
    meet(&watch, &phone);
    until("both connected to the phone", || {
        row_named(&phone, "Mac")["state"] == "up_to_date"
            && row_named(&phone, "Watch")["state"] == "up_to_date"
    })
    .await;
    // In the app, Sync writes through its own connection, which the change-log hook never sees:
    // the phone learns nothing of what it stores from the Mac unless the manager passes it on.
    phone
        .app
        .with_db(|c| mnemora_core::db::on_change_log_write(c, || {}).unwrap());
    tokio::time::sleep(Duration::from_millis(300)).await;

    let id = create(&mac, "Reaches the watch through the phone");
    until("the Task on the phone", || has(&phone, &id)).await;
    until("the Task on the watch", || has(&watch, &id)).await;
}

#[tokio::test]
async fn turning_sync_off_and_on_again_and_again_leaves_nothing_behind() {
    let (mac, phone) = paired_pair(quick()).await;
    meet(&mac, &phone);
    until("connected", || peer_of(&mac)["state"] == "up_to_date").await;
    let options = || mnemora_sync::NetworkOptions {
        ip: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        port: 0,
        ip6: None,
        timing: mnemora_sync::Timing::default(),
        discovery: mac.found.clone(),
        pace: quick(),
        local_network: mnemora_sync::local_network::LocalNetwork::off(),
    };
    for _ in 0..4 {
        mac.call::<()>("sync_set_enabled", json!({ "enabled": false }));
        assert_eq!(mac.app.sync_probe().tasks, 0);
        mac.call::<()>("sync_set_enabled", json!({ "enabled": true }));
        crate::sync::start_network(&mac.app.handle(), options())
            .await
            .unwrap();
        mac.found.announce(&phone.app.device_id(), phone.address());
        phone.found.announce(&mac.app.device_id(), mac.address());
    }
    until("connected again", || peer_of(&mac)["state"] == "up_to_date").await;
    // Only the supervisor, the accept loop and pairing's tasks, one connection: no pile-up.
    let running = mac.app.sync_probe().tasks;
    assert!(running <= 6, "{running} tasks after four restarts");
    mac.call::<()>("sync_set_enabled", json!({ "enabled": false }));
    assert_eq!(mac.app.sync_probe().tasks, 0);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(mac.found.active(), 0, "announcing or browsing is left over");
}

/// The Activity lines of a device, as `kind -> lines`.
pub(crate) fn activity_lines(device: &Device, kind: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page: Value = device.call(
            "sync_activity",
            json!({ "filter": { "kind": "all" }, "cursor": cursor, "limit": 100 }),
        );
        for row in page["rows"].as_array().unwrap() {
            if row["kind"] == kind {
                let text: String = row["text"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| p["text"].as_str().unwrap())
                    .collect();
                lines.push(text);
            }
        }
        match page["next"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => return lines,
        }
    }
}

#[tokio::test]
async fn pairing_and_a_live_session_are_each_one_line_on_both_devices() {
    let (mac, phone) = paired_pair(quick()).await;
    meet(&mac, &phone);
    until("connected", || {
        peer_of(&mac)["state"] == "up_to_date" && peer_of(&phone)["state"] == "up_to_date"
    })
    .await;
    let id = create(&mac, "Plan weekend hike");
    until("the task on the phone", || has(&phone, &id)).await;
    // The line a session writes travels on by itself: both devices end with the same two lines.
    until("both lines on both devices", || {
        [&mac, &phone].iter().all(|d| {
            activity_lines(d, "paired").len() == 1 && activity_lines(d, "session").len() == 1
        })
    })
    .await;
    let (lead, follow) = if mac.app.device_id() < phone.app.device_id() {
        (&mac, "Pixel 8")
    } else {
        (&phone, "Mac")
    };
    assert_eq!(
        activity_lines(lead, "paired"),
        [format!("Paired with {follow}")]
    );
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(
        activity_lines(&mac, "session").len(),
        1,
        "empty rounds are not logged"
    );
    assert_eq!(activity_lines(&phone, "session").len(), 1);
}

/// A rename is a record like any other change: it goes to the connected device after the quiet
/// period, not at the next session (DV5).
#[tokio::test]
async fn renaming_a_device_reaches_it_within_the_quiet_period() {
    let (mac, phone) = paired_pair(quick()).await;
    meet(&mac, &phone);
    until("connected", || peer_of(&mac)["state"] == "up_to_date").await;
    // Pairing's own sessions and quiet period run out first.
    tokio::time::sleep(Duration::from_millis(1_500)).await;

    let phone_id = phone.app.device_id();
    let made = Instant::now();
    mac.app
        .invoke::<String>(
            "sync_device_rename",
            json!({ "id": phone_id, "name": "Pixel QA" }),
        )
        .unwrap();
    until("the new name on the phone", || {
        phone.app.invoke::<Value>("sync_get", json!({})).unwrap()["name"] == "Pixel QA"
    })
    .await;
    assert_eq!(phone.devices()["this_device"]["name"], "Pixel QA");
    assert!(
        made.elapsed() < Duration::from_secs(4),
        "the rename waited for a later session: {:?}",
        made.elapsed()
    );
}

/// The Activity line about a rename never shows before the new name is applied (DV8): whenever the
/// line is there, the device already has its new name.
#[tokio::test]
async fn the_rename_line_never_shows_before_the_new_name_is_applied() {
    for _ in 0..5 {
        let (mac, phone) = paired_pair(quick()).await;
        meet(&mac, &phone);
        until("connected", || peer_of(&mac)["state"] == "up_to_date").await;
        tokio::time::sleep(Duration::from_millis(1_500)).await;
        mac.app
            .invoke::<String>(
                "sync_device_rename",
                json!({ "id": phone.app.device_id(), "name": "Pixel QA" }),
            )
            .unwrap();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(8) {
            let line = activity_lines(&phone, "renamed");
            let named =
                phone.app.invoke::<Value>("sync_get", json!({})).unwrap()["name"] == "Pixel QA";
            assert!(
                line.is_empty() || named,
                "the line {line:?} showed before the name was applied"
            );
            if named {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        until("the line", || !activity_lines(&phone, "renamed").is_empty()).await;
    }
}

/// The Mac that macOS keeps off the local network finds nobody (mDNS is blocked) yet is still
/// synced: the other device dials the address it was given, and the Mac only accepts (D14).
#[tokio::test]
async fn a_device_that_finds_nobody_still_syncs_when_the_other_dials_its_typed_address() {
    let (mac, phone) = paired_pair(quick()).await;
    // No `meet`: neither discovery announces the other. Nothing connects by itself.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(peer_of(&phone)["state"], "not_nearby");
    let mac_id = mac.app.device_id();
    phone
        .app
        .invoke::<Value>(
            "sync_device_address",
            json!({ "id": mac_id, "address": mac.address().to_string() }),
        )
        .unwrap();
    until("both connected through the phone's dial", || {
        peer_of(&mac)["state"] == "up_to_date" && peer_of(&phone)["state"] == "up_to_date"
    })
    .await;
}

/// What the Devices tab, the sheet and the Home card say while the system keeps the device off the
/// local network (D13): reachable devices stay as they are, the others say why.
#[tokio::test]
async fn the_local_network_being_blocked_is_told_in_the_rows_the_card_and_attention() {
    let (mac, phone) = paired_pair(quick()).await;
    let view = || phone.devices();
    assert_eq!(view()["this_device"]["local_network_blocked"], false);
    assert_eq!(peer_of(&phone)["state"], "not_nearby");
    let attention = || {
        phone
            .app
            .invoke::<Value>("sync_attention", json!({}))
            .unwrap()
    };
    assert_eq!(attention()["local_network_blocked"], false);

    // The system says no (Android's permission): it shows at once, nothing waits for a dial.
    phone
        .app
        .invoke::<()>("sync_local_network_os", json!({ "denied": true }))
        .unwrap();
    assert_eq!(view()["this_device"]["local_network_blocked"], true);
    assert_eq!(peer_of(&phone)["state"], "blocked_by_local_network");
    let a = attention();
    assert_eq!(a["local_network_blocked"], true);
    assert_eq!(a["needed"], true);
    let id = mac.app.device_id();
    let sheet = phone
        .app
        .invoke::<Value>("sync_device", json!({ "id": id }))
        .unwrap();
    assert_eq!(sheet["state"], "blocked_by_local_network");

    // A device that is connected anyway (it dialled us) is reachable, not blocked.
    meet(&mac, &phone);
    until("connected", || peer_of(&phone)["state"] == "up_to_date").await;

    phone
        .app
        .invoke::<()>("sync_local_network_os", json!({ "denied": false }))
        .unwrap();
    assert_eq!(view()["this_device"]["local_network_blocked"], false);
    assert_eq!(attention()["local_network_blocked"], false);
}

/// Nothing is asked of, or blamed on, the system while Sync is off.
#[tokio::test]
async fn a_denial_reported_while_sync_is_off_is_ignored() {
    let (_mac, phone) = paired_pair(quick()).await;
    phone.call::<()>("sync_set_enabled", json!({ "enabled": false }));
    phone
        .app
        .invoke::<()>("sync_local_network_os", json!({ "denied": true }))
        .unwrap();
    phone.call::<()>("sync_set_enabled", json!({ "enabled": true }));
    assert_eq!(
        phone.devices()["this_device"]["local_network_blocked"],
        false
    );
}

/// A refusal that was true while Sync was on is not shown once it is off.
#[tokio::test]
async fn turning_sync_off_clears_the_local_network_warning() {
    let (_mac, phone) = paired_pair(quick()).await;
    phone
        .app
        .invoke::<()>("sync_local_network_os", json!({ "denied": true }))
        .unwrap();
    assert_eq!(
        phone.devices()["this_device"]["local_network_blocked"],
        true
    );
    phone.call::<()>("sync_set_enabled", json!({ "enabled": false }));
    assert_eq!(
        phone.devices()["this_device"]["local_network_blocked"],
        false
    );
    let attention = phone
        .app
        .invoke::<Value>("sync_attention", json!({}))
        .unwrap();
    assert_eq!(attention["local_network_blocked"], false);
}
