//! Sync commands. Thin: the rules live in `mnemora-sync`.

use mnemora_core::error::AppError;
use mnemora_sync::{SyncOption, SyncState};
use tauri::State;

use crate::state::{AppState, lock};

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
pub fn sync_set_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), AppError> {
    mnemora_sync::set_enabled(&state.device, &state.sync, enabled)
}

/// Renames this device (1 to 40 characters).
#[tauri::command]
#[specta::specta]
pub fn sync_rename(state: State<'_, AppState>, name: String) -> Result<(), AppError> {
    mnemora_sync::rename(&state.device, &name)
}

/// Sets "Sync automatically", "Allow over mobile data" or "Visible on this network".
#[tauri::command]
#[specta::specta]
pub fn sync_set_option(
    state: State<'_, AppState>,
    option: SyncOption,
    enabled: bool,
) -> Result<(), AppError> {
    mnemora_sync::set_option(&state.device, option, enabled)
}

/// Leaves the group: forgets every paired device. This device keeps its data.
#[tauri::command]
#[specta::specta]
pub fn sync_leave(state: State<'_, AppState>) -> Result<(), AppError> {
    mnemora_sync::forget_all(&lock(&state.db))
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
