//! Pairing between two `TestApp`s over real loopback sockets: the commands the UI calls, in the
//! order it calls them, and what each device lists afterwards (ADR 0009).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use mnemora_sync::discovery::Fixed;
use mnemora_sync::{NetworkOptions, Pace, Timing};
use serde_json::{Value, json};

use crate::sync::start_network;
use crate::test_support::TestApp;

/// A device with Sync on, a name, and its network up on a loopback port.
pub(crate) struct Device {
    pub(crate) app: TestApp,
    pub(crate) found: Arc<Fixed>,
}

impl Device {
    pub(crate) async fn new(name: &str) -> Self {
        Self::with_pace(name, Pace::default()).await
    }

    pub(crate) async fn with_pace(name: &str, pace: Pace) -> Self {
        Self::build(name, pace, None).await
    }

    /// Also listening on the IPv6 loopback, on the port of the IPv4 one.
    async fn dual_stack(name: &str) -> Self {
        Self::build(name, Pace::default(), Some(std::net::Ipv6Addr::LOCALHOST)).await
    }

    async fn build(name: &str, pace: Pace, ip6: Option<std::net::Ipv6Addr>) -> Self {
        let app = TestApp::syncing();
        app.invoke::<()>("sync_rename", json!({ "name": name }))
            .unwrap();
        let found = Fixed::new();
        let options = NetworkOptions {
            ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 0,
            ip6,
            timing: Timing {
                search: Duration::from_millis(800),
                ..Timing::default()
            },
            discovery: found.clone(),
            pace,
            local_network: mnemora_sync::local_network::LocalNetwork::off(),
        };
        start_network(&app.handle(), options).await.unwrap();
        Self { app, found }
    }

    pub(crate) fn call<T: serde::de::DeserializeOwned>(&self, command: &str, args: Value) -> T {
        self.app.invoke(command, args).unwrap()
    }

    pub(crate) fn pairing(&self) -> Value {
        self.call("sync_pair_state", json!({}))
    }

    pub(crate) async fn until(&self, state: &str) -> Value {
        // Generous: the whole suite runs these in parallel on a small machine.
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(20) {
            let view = self.pairing();
            if view["state"] == state {
                return view;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("never reached {state}; at {}", self.pairing());
    }

    pub(crate) fn devices(&self) -> Value {
        self.call("sync_devices", json!({}))
    }

    pub(crate) fn listed(&self) -> Vec<String> {
        self.devices()["peers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap().to_owned())
            .collect()
    }

    pub(crate) fn address(&self) -> SocketAddr {
        let port = self.devices()["this_device"]["port"].as_u64().unwrap();
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            u16::try_from(port).unwrap(),
        )
    }
}

#[tokio::test]
async fn after_pairing_both_devices_list_each_other() {
    let (mac, phone) = (
        Device::new("Hemang's MacBook Pro").await,
        Device::new("Pixel 8").await,
    );
    assert_eq!(mac.listed(), Vec::<String>::new());

    mac.call::<()>("sync_pair_show", json!({}));
    let showing = mac.pairing();
    assert_eq!(showing["state"], "showing");
    let code = showing["code"].as_str().unwrap().to_owned();
    assert_eq!(code.len(), 9, "two groups of four: {code}");

    phone.found.set(vec![mac.address()]);
    phone.call::<()>("sync_pair_enter", json!({ "code": code }));
    let asked = mac.until("asking").await;
    assert_eq!(asked["name"], "Pixel 8");
    assert_eq!(phone.until("waiting").await["name"], "Hemang's MacBook Pro");
    mac.call::<()>("sync_pair_decide", json!({ "allow": true }));

    assert_eq!(mac.until("paired").await["name"], "Pixel 8");
    assert_eq!(phone.until("paired").await["name"], "Hemang's MacBook Pro");
    assert_eq!(mac.listed(), ["Pixel 8"]);
    assert_eq!(phone.listed(), ["Hemang's MacBook Pro"]);
    assert_eq!(
        mac.call::<Value>("sync_get", json!({}))["paired_devices"],
        1
    );
    assert_eq!(
        phone.call::<Value>("sync_get", json!({}))["paired_devices"],
        1
    );

    // Leaving forgets the other and stops trusting it.
    mac.call::<()>("sync_leave", json!({}));
    assert_eq!(mac.listed(), Vec::<String>::new());
    assert_eq!(
        phone.listed(),
        ["Hemang's MacBook Pro"],
        "only this device left"
    );
}

#[tokio::test]
async fn a_wrong_code_shows_tries_left_through_the_commands() {
    let (mac, phone) = (Device::new("Mac").await, Device::new("Pixel 8").await);
    mac.call::<()>("sync_pair_show", json!({}));
    let code = mac.pairing()["code"].as_str().unwrap().replace(' ', "");
    let wrong = if code == "00000000" {
        "11111111"
    } else {
        "00000000"
    };
    phone.found.set(vec![mac.address()]);
    phone.call::<()>("sync_pair_enter", json!({ "code": wrong }));
    let failed = phone.until("failed").await;
    assert_eq!(failed["error"]["kind"], "wrong_code");
    assert_eq!(failed["error"]["tries_left"], 2);
    assert_eq!(failed["error"]["name"], "Mac");
    let err = phone
        .app
        .invoke::<()>("sync_pair_enter", json!({ "code": "123" }))
        .unwrap_err();
    assert_eq!(err["code"], "validation");
}

#[tokio::test]
async fn the_network_exists_only_while_sync_is_on() {
    let mac = Device::new("Mac").await;
    let probe = mac.app.sync_probe();
    assert!(
        probe.running && probe.listeners == 1 && probe.advertisements == 2 /* the announcement and the paired-device browse */ && probe.tasks >= 1,
        "{probe:?}"
    );
    let addr = mac.address();
    mac.call::<()>("sync_pair_show", json!({}));

    mac.call::<()>("sync_set_enabled", json!({ "enabled": false }));
    assert_eq!(
        mac.app.sync_probe(),
        mnemora_sync::RuntimeProbe {
            running: false,
            listeners: 0,
            advertisements: 0,
            tasks: 0,
            sessions: 0
        }
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        tokio::net::TcpStream::connect(addr).await.is_err(),
        "the port is still open"
    );
    // Pairing cannot be driven while off.
    assert!(mac.app.invoke::<()>("sync_pair_show", json!({})).is_err());
    assert_eq!(mac.pairing()["state"], "idle");
}

#[test]
fn pairing_commands_are_refused_before_the_network_is_up() {
    let app = TestApp::syncing();
    let err = app.invoke::<()>("sync_pair_show", json!({})).unwrap_err();
    assert_eq!(err["code"], "validation");
    assert_eq!(
        app.invoke::<Value>("sync_pair_state", json!({})).unwrap()["state"],
        "idle"
    );
}

#[tokio::test]
async fn pairing_by_address_needs_no_discovery_and_remembers_the_address() {
    use crate::sync_live_tests::{peer_of, quick, until};
    let (mac, phone) = (
        Device::with_pace("Mac", quick()).await,
        Device::with_pace("Pixel 8", quick()).await,
    );
    mac.call::<()>("sync_pair_show", json!({}));
    let code = mac.pairing()["code"].as_str().unwrap().to_owned();
    // The phone finds nothing on the network: only the typed address leads to the Mac.
    let address = mac.address().to_string();
    phone.call::<()>(
        "sync_pair_enter_at",
        json!({ "address": address, "code": code }),
    );
    assert_eq!(mac.until("asking").await["name"], "Pixel 8");
    mac.call::<()>("sync_pair_decide", json!({ "allow": true }));
    assert_eq!(phone.until("paired").await["name"], "Mac");
    assert_eq!(mac.until("paired").await["name"], "Pixel 8");

    let mac_id = phone.devices()["peers"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let sheet: Value = phone.call("sync_device", json!({ "id": mac_id }));
    assert_eq!(sheet["typed_address"], json!(address));
    // The Mac showed the code, so it dialled nothing: the typed Address is what connects them.
    until("connected by the typed address", || {
        peer_of(&phone)["state"] == "up_to_date"
    })
    .await;
}

#[tokio::test]
async fn pairing_by_address_says_why_it_did_not_work() {
    let (mac, phone) = (Device::new("Mac").await, Device::new("Pixel 8").await);
    for typed in ["pixel 8", "bad address!!"] {
        let invalid = phone
            .app
            .invoke::<()>(
                "sync_pair_enter_at",
                json!({ "address": typed, "code": "12345678" }),
            )
            .unwrap_err();
        assert_eq!(invalid["code"], "validation", "{typed}");
        assert_eq!(
            invalid["message"],
            "That isn’t a host name or an IP address"
        );
        // Refused before anything was dialled: the sheet is not left "searching".
        assert_eq!(phone.pairing()["state"], "idle", "{typed}");
    }
    let short = phone
        .app
        .invoke::<()>(
            "sync_pair_enter_at",
            json!({ "address": "127.0.0.1", "code": "123" }),
        )
        .unwrap_err();
    assert_eq!(short["code"], "validation");

    // Nothing listens there.
    let closed = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    phone.call::<()>(
        "sync_pair_enter_at",
        json!({ "address": closed.to_string(), "code": "12345678" }),
    );
    let failed = phone.until("failed").await;
    assert_eq!(failed["error"]["kind"], "address_unreachable");

    // A device that shows a code, and a wrong one: the usual pairing error, naming the device.
    mac.call::<()>("sync_pair_show", json!({}));
    let code = mac.pairing()["code"].as_str().unwrap().replace(' ', "");
    let wrong = if code == "00000000" {
        "11111111"
    } else {
        "00000000"
    };
    phone.call::<()>(
        "sync_pair_enter_at",
        json!({ "address": mac.address().to_string(), "code": wrong }),
    );
    let failed = phone.until("failed").await;
    assert_eq!(failed["error"]["kind"], "wrong_code");
    assert_eq!(failed["error"]["tries_left"], 2);
    assert_eq!(phone.listed(), Vec::<String>::new());
}

#[tokio::test]
async fn a_device_can_be_paired_over_its_ipv6_address() {
    let (mac, phone) = (
        Device::dual_stack("Mac").await,
        Device::new("Pixel 8").await,
    );
    mac.call::<()>("sync_pair_show", json!({}));
    let code = mac.pairing()["code"].as_str().unwrap().to_owned();
    let address = format!("[::1]:{}", mac.address().port());
    phone.call::<()>(
        "sync_pair_enter_at",
        json!({ "address": address, "code": code }),
    );
    assert_eq!(mac.until("asking").await["name"], "Pixel 8");
    mac.call::<()>("sync_pair_decide", json!({ "allow": true }));
    assert_eq!(phone.until("paired").await["name"], "Mac");
}

#[tokio::test]
async fn sync_off_closes_the_ipv6_port_too() {
    let mac = Device::dual_stack("Mac").await;
    let port = mac.address().port();
    mac.call::<()>("sync_set_enabled", json!({ "enabled": false }));
    assert_eq!(mac.app.sync_probe().tasks, 0);
    let v6 = std::net::SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, port));
    // The listener tasks are aborted, which takes effect at their next poll.
    crate::sync_live_tests::until("the IPv6 port to close", || {
        std::net::TcpStream::connect_timeout(&v6, Duration::from_millis(200)).is_err()
    })
    .await;
}
