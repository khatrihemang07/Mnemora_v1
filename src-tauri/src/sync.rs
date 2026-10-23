//! Sync commands. Thin: the rules live in `mnemora-sync`.

use std::sync::{Arc, Mutex};

use mnemora_core::db::{self, Connection, migrate};
use mnemora_core::error::AppError;
use mnemora_core::events::EventBus;
use mnemora_sync::discovery::{Discovery, Fixed, Mdns};
use mnemora_sync::net::{DeviceIdentity, Handshake, PublicKey};
use mnemora_sync::{
    DevicesView, Host, Network, NetworkOptions, Paired, PairingHost, PairingView, Platform,
    Registry, Stored, SyncError, SyncOption, SyncState, Who,
};
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_specta::Event;

use crate::state::{AppState, lock};
use crate::{SyncActivityEvent, SyncPairingEvent, SyncPeersEvent};

/// Sync's state for this device: on or off, its name and options, and how many devices are paired.
#[tauri::command]
#[specta::specta]
pub fn sync_get(state: State<'_, AppState>) -> Result<SyncState, AppError> {
    mnemora_sync::current(&state.device.get(), &lock(&state.db))
}

/// Turns Sync on or off for this device only (never synced). Off stops everything Sync runs and
/// keeps the data and the paired devices.
#[tauri::command]
#[specta::specta]
pub fn sync_set_enabled<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), AppError> {
    mnemora_sync::set_enabled(&state.device, &state.sync, enabled)?;
    if enabled {
        launch_network(&app);
    }
    Ok(())
}

/// Renames this device (1 to 40 characters).
#[tauri::command]
#[specta::specta]
pub fn sync_rename(state: State<'_, AppState>, name: String) -> Result<(), AppError> {
    let old = state
        .device
        .get()
        .sync
        .name
        .unwrap_or_else(mnemora_sync::default_device_name);
    mnemora_sync::rename(&state.device, &name)?;
    state.sync.refresh_advert();
    // A device that has no group yet has nobody to tell.
    let new = state
        .device
        .get()
        .sync
        .name
        .unwrap_or_else(mnemora_sync::default_device_name);
    let conn = lock(&state.db);
    if old != new && mnemora_sync::group(&conn)?.is_some() {
        mnemora_sync::log_renamed(&conn, &state.device.device_id(), &old, &new)?;
    }
    Ok(())
}

/// Sets "Sync automatically", "Allow over mobile data" or "Visible on this network".
#[tauri::command]
#[specta::specta]
pub fn sync_set_option(
    state: State<'_, AppState>,
    option: SyncOption,
    enabled: bool,
) -> Result<(), AppError> {
    mnemora_sync::set_option(&state.device, option, enabled)?;
    match option {
        SyncOption::Visible => state.sync.refresh_advert(),
        SyncOption::Automatic => state.sync.options_changed(),
        SyncOption::MobileData | SyncOption::ToastConflicts => {}
    }
    Ok(())
}

/// Leaves the group: forgets every paired device. This device keeps its data.
#[tauri::command]
#[specta::specta]
pub fn sync_leave(state: State<'_, AppState>) -> Result<(), AppError> {
    mnemora_sync::forget_all(&lock(&state.db))?;
    state.sync.unpin_all();
    state.sync.peers_changed();
    state.sync.refresh_advert();
    Ok(())
}

/// The Devices tab: this device (name, addresses, port) and the paired ones.
#[tauri::command]
#[specta::specta]
pub fn sync_devices(state: State<'_, AppState>) -> Result<DevicesView, AppError> {
    mnemora_sync::devices_view(
        &state.device.get(),
        &lock(&state.db),
        state.sync.listening(),
        &state.sync.peers_live(),
    )
}

/// "Sync now": syncs with every reachable paired device now, even with Sync automatically off.
/// Returns at once; each device's row shows how it goes.
#[tauri::command]
#[specta::specta]
pub fn sync_now(state: State<'_, AppState>) {
    state.sync.sync_now();
}

/// The network changed or the app came back to the foreground: devices that could not be reached
/// are tried again from the start of the retry schedule.
#[tauri::command]
#[specta::specta]
pub fn sync_resume(state: State<'_, AppState>) {
    // An editor that went away with the app in the background no longer holds changes back.
    crate::editing::release_lapsed(&state);
    state.sync.resume();
}

/// Where pairing stands. Idle while Sync is off or still starting.
#[tauri::command]
#[specta::specta]
pub fn sync_pair_state(state: State<'_, AppState>) -> PairingView {
    state
        .sync
        .pairing()
        .map_or(PairingView::Idle, |p| p.state())
}

/// "Pair a device": shows a code and waits for the other device to type it.
#[tauri::command]
#[specta::specta]
pub fn sync_pair_show(state: State<'_, AppState>) -> Result<(), AppError> {
    Ok(state.sync.pairing()?.show()?)
}

/// "New code": a fresh code with 3 tries and 2 minutes.
#[tauri::command]
#[specta::specta]
pub fn sync_pair_new_code(state: State<'_, AppState>) -> Result<(), AppError> {
    Ok(state.sync.pairing()?.new_code()?)
}

/// "Enter a code": looks for a device showing a code on the network and tries the typed one.
#[tauri::command]
#[specta::specta]
pub fn sync_pair_enter(state: State<'_, AppState>, code: String) -> Result<(), AppError> {
    Ok(state.sync.pairing()?.enter(&code)?)
}

/// Allow or Deny the device that typed the right code.
#[tauri::command]
#[specta::specta]
pub fn sync_pair_decide(state: State<'_, AppState>, allow: bool) -> Result<(), AppError> {
    state.sync.pairing()?.decide(allow);
    Ok(())
}

/// Closes the pairing sheet's pairing: no code, no search; whoever was asked is told.
#[tauri::command]
#[specta::specta]
pub fn sync_pair_cancel(state: State<'_, AppState>) {
    if let Ok(pairing) = state.sync.pairing() {
        pairing.cancel();
    }
}

/// The app side of pairing: this device's facts from the database and device file, the pairing
/// written to the database, and the pairing state sent to the window as an event.
pub struct AppHost<R: Runtime> {
    app: AppHandle<R>,
    /// Sync's own connection to the database file, so a big sync never holds a command up and
    /// a command never holds a sync up. `None` where the database is in memory (tests), which
    /// can only be reached through the app's one connection.
    own_db: Option<Mutex<Connection>>,
}

impl<R: Runtime> AppHost<R> {
    pub fn new(app: AppHandle<R>, own_connection: bool) -> Result<Self, SyncError> {
        let own_db = if own_connection {
            let state = app.state::<AppState>();
            Some(Mutex::new(db::open(&state.paths.db_file)?))
        } else {
            None
        };
        Ok(Self { app, own_db })
    }
}

impl<R: Runtime> PairingHost for AppHost<R> {
    fn me(&self) -> Result<Who, SyncError> {
        let state = self.app.state::<AppState>();
        let conn = lock(&state.db);
        let device = state.device.get();
        Ok(Who {
            device: state.device.device_id(),
            name: device
                .sync
                .name
                .clone()
                .unwrap_or_else(mnemora_sync::default_device_name),
            platform: Platform::this(),
            key: String::new(),
            environment: state.env,
            group: mnemora_sync::group(&conn)?,
            data_versions: migrate::versions(&conn)?,
            members: mnemora_sync::members(&conn)?,
        })
    }

    fn handshake(&self) -> Result<Handshake, SyncError> {
        let state = self.app.state::<AppState>();
        let conn = lock(&state.db);
        let group = mnemora_sync::group(&conn)?;
        Handshake::local(
            &conn,
            state.env,
            &state.device.device_id(),
            group,
            Vec::new(),
        )
    }

    fn visible(&self) -> bool {
        self.app.state::<AppState>().device.get().sync.visible
    }

    fn pinned(&self) -> Vec<(PublicKey, String)> {
        let state = self.app.state::<AppState>();
        mnemora_sync::pinned(&lock(&state.db)).unwrap_or_default()
    }

    fn store(&self, paired: &Paired) -> Result<Stored, SyncError> {
        let state = self.app.state::<AppState>();
        let name = state
            .device
            .get()
            .sync
            .name
            .clone()
            .unwrap_or_else(mnemora_sync::default_device_name);
        let device = state.device.device_id();
        mnemora_sync::store_paired(&mut lock(&state.db), &device, &name, paired)
    }

    fn show(&self, view: &PairingView) {
        if let Err(e) = SyncPairingEvent(view.clone()).emit(&self.app) {
            tracing::warn!(target: "sync", error = %e, "could not emit the pairing state");
        }
    }

    fn paired(&self) {
        self.app.state::<AppState>().sync.peers_changed();
    }
}

impl<R: Runtime> Host for AppHost<R> {
    fn with_db(
        &self,
        f: &mut dyn FnMut(&Connection) -> Result<(), SyncError>,
    ) -> Result<(), SyncError> {
        match &self.own_db {
            Some(own) => f(&lock(own)),
            None => f(&lock(&self.app.state::<AppState>().db)),
        }
    }

    fn registry(&self) -> Registry {
        Registry::new(Arc::clone(&self.app.state::<AppState>().modules))
    }

    fn bus(&self) -> EventBus {
        self.app.state::<AppState>().bus.clone()
    }

    fn automatic(&self) -> bool {
        self.app.state::<AppState>().device.get().sync.automatic
    }

    fn local_changes(&self) -> Arc<mnemora_sync::Notify> {
        self.app.state::<AppState>().sync.local_changes()
    }

    fn status_changed(&self) {
        if let Err(e) = SyncPeersEvent.emit(&self.app) {
            tracing::warn!(target: "sync", error = %e, "could not emit the device status");
        }
    }

    fn activity_changed(&self) {
        if let Err(e) = SyncActivityEvent.emit(&self.app) {
            tracing::warn!(target: "sync", error = %e, "could not emit the Activity change");
        }
    }

    fn device_name(&self) -> String {
        self.app
            .state::<AppState>()
            .device
            .get()
            .sync
            .name
            .unwrap_or_else(mnemora_sync::default_device_name)
    }
}

/// Starts the network side of Sync: this device's key, the listener, discovery and pairing. A no-op
/// where the app does not run a live network (tests start their own).
pub fn launch_network<R: Runtime>(app: &AppHandle<R>) {
    if !app.state::<AppState>().live_network {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let discovery: Arc<dyn Discovery> = match Mdns::new(state.env) {
            Ok(mdns) => Arc::new(mdns),
            Err(e) => {
                // Pairing by address and paired devices still work; only finding is lost.
                tracing::warn!(target: "sync", error = %e, "local network discovery is unavailable");
                Fixed::new()
            }
        };
        if let Err(e) = start_network(&app, NetworkOptions::app(discovery)).await {
            tracing::warn!(target: "sync", error = %e, "could not start Sync's network");
        }
    });
}

/// Binds the port and hands the running [`Network`] to the runtime, unless Sync was turned off
/// while it started. Needs a Tokio runtime.
pub async fn start_network<R: Runtime>(
    app: &AppHandle<R>,
    options: NetworkOptions,
) -> Result<(), SyncError> {
    let state = app.state::<AppState>();
    if !state.device.get().sync.enabled {
        return Err(SyncError::Off);
    }
    let dir = state
        .paths
        .device_file()
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let identity = Arc::new(DeviceIdentity::load_or_create(&dir).map_err(SyncError::from)?);
    let host = Arc::new(AppHost::new(app.clone(), state.live_network)?);
    let network = Network::start(host, identity, options).await?;
    // Turned off meanwhile: the network has no owner and is dropped here, closing its port.
    let _ = state.sync.attach(network);
    Ok(())
}

#[cfg(test)]
mod tests {
    use mnemora_core::device::DeviceSettings;
    use mnemora_sync::RuntimeProbe;
    use serde_json::{Value, json};

    use crate::test_support::TestApp;

    const NOTHING: RuntimeProbe = RuntimeProbe {
        running: false,
        listeners: 0,
        advertisements: 0,
        tasks: 0,
        sessions: 0,
    };

    fn get(app: &TestApp) -> Value {
        app.invoke("sync_get", json!({})).unwrap()
    }

    fn device(app: &TestApp) -> Value {
        app.invoke("device_get", json!({})).unwrap()
    }

    fn set_enabled(app: &TestApp, enabled: bool) {
        app.invoke::<()>("sync_set_enabled", json!({ "enabled": enabled }))
            .unwrap();
    }

    fn sync_on_device() -> DeviceSettings {
        let mut d = DeviceSettings::default();
        d.sync.enabled = true;
        d
    }

    #[test]
    fn a_fresh_install_has_sync_off_and_nothing_running() {
        let app = TestApp::new();
        assert_eq!(get(&app)["enabled"], false);
        assert_eq!(app.sync_probe(), NOTHING, "no listener, mDNS or task");
        // Its tables are migrated on every install and stay empty.
        let peers: i64 = app.with_db(|c| {
            c.query_row("SELECT COUNT(*) FROM sync_peers", [], |r| r.get(0))
                .unwrap()
        });
        assert_eq!(peers, 0);
        assert_eq!(get(&app)["paired_devices"], 0);
    }

    #[test]
    fn existing_installs_get_sync_off_too() {
        // A device file from before Sync, with other choices already made.
        let before_sync = DeviceSettings {
            spellcheck: false,
            ..DeviceSettings::default()
        };
        let app = TestApp::upgrading(|_| {}, before_sync);
        assert_eq!(get(&app)["enabled"], false);
        assert_eq!(app.sync_probe(), NOTHING);
    }

    #[test]
    fn nothing_runs_while_off_and_the_services_follow_the_switch() {
        let app = TestApp::new();
        assert_eq!(app.sync_probe(), NOTHING);
        set_enabled(&app, true);
        assert!(app.sync_probe().running, "turning it on starts Sync");
        set_enabled(&app, false);
        assert_eq!(app.sync_probe(), NOTHING, "turning it off stops everything");
        set_enabled(&app, false);
        assert_eq!(app.sync_probe(), NOTHING, "off twice is still off");
    }

    #[test]
    fn a_restart_with_sync_on_starts_it_and_with_it_off_does_not() {
        let on = TestApp::upgrading(|_| {}, sync_on_device());
        assert!(on.sync_probe().running);
        let off = TestApp::upgrading(|_| {}, DeviceSettings::default());
        assert_eq!(off.sync_probe(), NOTHING);
    }

    #[test]
    fn on_off_is_a_device_setting_never_a_synced_preference() {
        let app = TestApp::new();
        let changes_before: i64 = app.with_db(|c| {
            c.query_row("SELECT COUNT(*) FROM change_log", [], |r| r.get(0))
                .unwrap()
        });
        set_enabled(&app, true);
        assert_eq!(device(&app)["sync"]["enabled"], true, "kept in device.toml");
        let settings: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(
            settings["values"]["core.enabled_optional_modules"],
            json!([])
        );
        assert!(
            !settings["values"].to_string().contains("sync"),
            "no Preference mentions Sync: {settings}"
        );
        let (rows, changes_after): (i64, i64) = app.with_db(|c| {
            (
                c.query_row(
                    "SELECT COUNT(*) FROM settings WHERE module = 'sync'",
                    [],
                    |r| r.get(0),
                )
                .unwrap(),
                c.query_row("SELECT COUNT(*) FROM change_log", [], |r| r.get(0))
                    .unwrap(),
            )
        });
        assert_eq!(rows, 0);
        assert_eq!(
            changes_after, changes_before,
            "nothing to sync was recorded"
        );
    }

    #[test]
    fn disabling_another_module_is_still_a_synced_preference() {
        let app = TestApp::new();
        app.set_pref("core.disabled_modules", json!(["todo"]));
        let recorded: i64 = app.with_db(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM change_log WHERE entity = 'setting' AND entity_id = 'core.disabled_modules'",
                [],
                |r| r.get(0),
            )
            .unwrap()
        });
        assert_eq!(recorded, 1);
    }

    #[test]
    fn turning_sync_on_names_the_device_once() {
        let app = TestApp::new();
        set_enabled(&app, true);
        let first = get(&app)["name"].as_str().unwrap().to_owned();
        assert!(!first.is_empty() && first.chars().count() <= 40, "{first}");
        assert_eq!(device(&app)["sync"]["name"], first, "saved, not recomputed");
        app.invoke::<()>("sync_rename", json!({ "name": "Studio Mac" }))
            .unwrap();
        set_enabled(&app, false);
        set_enabled(&app, true);
        assert_eq!(get(&app)["name"], "Studio Mac", "kept across off and on");
    }

    #[test]
    fn a_name_is_trimmed_and_must_be_one_to_forty_characters() {
        let app = TestApp::new();
        set_enabled(&app, true);
        app.invoke::<()>("sync_rename", json!({ "name": "  Pixel 8 " }))
            .unwrap();
        assert_eq!(get(&app)["name"], "Pixel 8");
        for bad in ["", "   ", &"a".repeat(41)] {
            let err = app
                .invoke::<()>("sync_rename", json!({ "name": bad }))
                .unwrap_err();
            assert_eq!(err["code"], "validation", "{bad:?}");
        }
        assert_eq!(
            get(&app)["name"],
            "Pixel 8",
            "a refused name changes nothing"
        );
        app.invoke::<()>("sync_rename", json!({ "name": "a".repeat(40) }))
            .unwrap();
    }

    #[test]
    fn the_options_default_as_specified_and_are_saved_per_device() {
        let app = TestApp::new();
        let s = get(&app);
        assert_eq!(
            (&s["automatic"], &s["mobile_data"], &s["visible"]),
            (&json!(true), &json!(false), &json!(true))
        );
        for (option, key) in [
            ("automatic", "automatic"),
            ("mobile_data", "mobile_data"),
            ("visible", "visible"),
        ] {
            let was = get(&app)[key].as_bool().unwrap();
            app.invoke::<()>(
                "sync_set_option",
                json!({ "option": option, "enabled": !was }),
            )
            .unwrap();
            assert_eq!(get(&app)[key], !was, "{option}");
            assert_eq!(device(&app)["sync"][key], !was, "{option} in device.toml");
        }
    }

    #[test]
    fn leave_forgets_every_paired_device_and_keeps_the_data() {
        let app = TestApp::new();
        set_enabled(&app, true);
        app.invoke::<Value>("todo_list_create", json!({ "name": "Work" }))
            .unwrap();
        app.with_db(|c| {
            c.execute_batch(
                "INSERT INTO sync_peers(id, name, paired_at) VALUES ('a', 'Pixel 8', 1), ('b', 'iPad', 2)",
            )
            .unwrap();
        });
        assert_eq!(get(&app)["paired_devices"], 2);
        app.invoke::<()>("sync_leave", json!({})).unwrap();
        assert_eq!(get(&app)["paired_devices"], 0);
        let lists: i64 = app.with_db(|c| {
            c.query_row("SELECT COUNT(*) FROM todo_lists", [], |r| r.get(0))
                .unwrap()
        });
        assert_eq!(lists, 1, "data is kept");
    }

    /// The device-name rules the browser mock also runs (`mock-sync.test.ts`).
    #[test]
    fn name_rules_match_the_shared_fixture() {
        const RULES: &str = include_str!("../../src/lib/ipc/fixtures/sync-rules.json");
        let rules: Value = serde_json::from_str(RULES).unwrap();
        for case in rules["name_cases"].as_array().unwrap() {
            let app = TestApp::new();
            let name = case["name"].as_str().unwrap();
            let result = app.invoke::<()>("sync_rename", json!({ "name": case["input"] }));
            if let Some(expected) = case.get("error") {
                let err = result.unwrap_err();
                assert_eq!(err["code"], expected["code"], "{name}");
                assert_eq!(err["message"], expected["message"], "{name}");
            } else {
                result.unwrap();
                assert_eq!(get(&app)["name"], case["saved"], "{name}");
            }
        }
    }
}
