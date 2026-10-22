//! IPC commands. Thin: validate nothing here that core already validates.

use mnemora_core::db::backup;
use mnemora_core::device::DeviceSettings;
use mnemora_core::env::Environment;
use mnemora_core::error::{AppError, ErrorCode};
use mnemora_core::logging::normalize_level;
use mnemora_core::settings::{SettingsChange, SettingsSnapshot};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_opener::OpenerExt;

use crate::state::{AppState, lock};

/// Sidebar width limits in pixels; below the minimum the UI snaps to the rail instead.
const SIDEBAR_MIN: u32 = 200;
const SIDEBAR_MAX: u32 = 360;
/// Cap on forwarded UI error text, so a runaway loop can't flood the log.
const UI_ERROR_MAX_CHARS: usize = 8_000;

/// Static facts about this running app.
#[derive(Debug, Clone, Serialize, Type)]
pub struct AppInfo {
    pub version: String,
    pub environment: Environment,
    pub data_dir: String,
    pub log_dir: String,
    /// First name from the OS account, for the greeting when no display name is set.
    pub os_first_name: String,
}

/// All Settings values plus the schema (defaults) of every key.
#[tauri::command]
#[specta::specta]
pub fn settings_get_all(state: State<'_, AppState>) -> Result<SettingsSnapshot, AppError> {
    state.settings.get_all(&lock(&state.db))
}

/// Saves a batch atomically. Emits `SettingsChangedEvent` on success.
#[tauri::command]
#[specta::specta]
pub fn settings_save(
    state: State<'_, AppState>,
    changes: Vec<SettingsChange>,
) -> Result<(), AppError> {
    state.settings.save_batch(&mut lock(&state.db), &changes)
}

/// Device-only settings (`device.toml`).
#[tauri::command]
#[specta::specta]
pub fn device_get(state: State<'_, AppState>) -> DeviceSettings {
    state.device.get()
}

/// Remembers the sidebar layout. Width is clamped to the allowed range.
#[tauri::command]
#[specta::specta]
pub fn device_set_sidebar(
    state: State<'_, AppState>,
    width: u32,
    rail: bool,
) -> Result<(), AppError> {
    state.device.update(|d| {
        d.sidebar.width = width.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
        d.sidebar.rail = rail;
    })
}

/// Remembers whether Todo's views and Lists are unfolded under "Todo" in the sidebar.
#[tauri::command]
#[specta::specta]
pub fn device_set_todo_open(state: State<'_, AppState>, open: bool) -> Result<(), AppError> {
    state.device.update(|d| d.sidebar.todo_open = Some(open))
}

/// Changes the log level immediately and remembers it.
#[tauri::command]
#[specta::specta]
pub fn device_set_log_level(state: State<'_, AppState>, level: String) -> Result<(), AppError> {
    let level = normalize_level(&level)?;
    state.log.set_level(&level)?;
    state.device.update(|d| d.log_level = level)
}

/// Turns spellcheck in every text editor on or off, and remembers it on this device.
#[tauri::command]
#[specta::specta]
pub fn device_set_spellcheck(state: State<'_, AppState>, enabled: bool) -> Result<(), AppError> {
    state.device.update(|d| d.spellcheck = enabled)
}

/// Version, environment, folders and the OS account's first name.
#[tauri::command]
#[specta::specta]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        environment: state.env,
        data_dir: state.paths.data_dir.display().to_string(),
        log_dir: state.paths.log_dir.display().to_string(),
        os_first_name: os_first_name(),
    }
}

/// Forwards a frontend error into the Rust log file.
#[tauri::command]
#[specta::specta]
pub fn log_ui_error(message: String, stack: Option<String>) {
    let message: String = message.chars().take(UI_ERROR_MAX_CHARS).collect();
    let stack: String = stack
        .unwrap_or_default()
        .chars()
        .take(UI_ERROR_MAX_CHARS)
        .collect();
    tracing::error!(target: "ui", %message, %stack, "unhandled UI error");
}

/// Takes a database backup now and returns its path.
#[tauri::command]
#[specta::specta]
pub fn backup_now(state: State<'_, AppState>) -> Result<String, AppError> {
    let path = backup::backup_now(&lock(&state.db), &state.paths.backup_dir)?;
    tracing::info!(path = %path.display(), "manual backup created");
    Ok(path.display().to_string())
}

/// Opens the log folder in the system file manager.
#[tauri::command]
#[specta::specta]
pub fn open_log_dir<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    app.opener()
        .open_path(state.paths.log_dir.display().to_string(), None::<&str>)
        .map_err(|e| AppError::new(ErrorCode::Io, "Could not open the log folder").with_details(e))
}

/// Names the OS reports when there is no real account name (Android returns "Anonymous").
const PLACEHOLDER_NAMES: [&str; 2] = ["anonymous", "unknown"];

/// First token of the name, or `None` when it is empty or a placeholder.
fn sanitize_first_name(name: &str) -> Option<String> {
    let first = name.split_whitespace().next()?;
    let lower = first.to_lowercase();
    (!PLACEHOLDER_NAMES.contains(&lower.as_str())).then(|| first.to_owned())
}

/// First token of the OS real name, else the account name; empty on phones or placeholders so
/// the UI shows a plain greeting.
fn os_first_name() -> String {
    if cfg!(mobile) {
        return String::new();
    }
    whoami::realname()
        .ok()
        .and_then(|n| sanitize_first_name(&n))
        .or_else(|| {
            whoami::username()
                .ok()
                .and_then(|n| sanitize_first_name(&n))
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use std::sync::Arc;

    use mnemora_core::db::{self, Connection};
    use mnemora_core::device::{DeviceSettings, DeviceStore, LookAndFeel};
    use mnemora_core::events::EventBus;
    use mnemora_core::module::{CoreModule, Module};
    use mnemora_core::settings::SettingsChange;
    use mnemora_journal::JournalModule;
    use mnemora_todo::TodoModule;

    use super::sanitize_first_name;
    use crate::test_support::TestApp;

    fn save_theme(app: &TestApp, value: Value) -> Result<(), Value> {
        app.invoke(
            "settings_save",
            json!({ "changes": [{ "key": "core.theme", "value": value }] }),
        )
    }

    fn get_all(app: &TestApp) -> Value {
        app.invoke("settings_get_all", json!({})).unwrap()
    }

    fn device(app: &TestApp) -> Value {
        app.invoke("device_get", json!({})).unwrap()
    }

    /// Rows of the `settings` table, and `setting` records in the change log, for `module.key`.
    fn stored_rows(app: &TestApp, key: &str) -> (i64, i64) {
        let (module, name) = key.split_once('.').unwrap();
        app.with_db(|c| {
            let rows = c
                .query_row(
                    "SELECT COUNT(*) FROM settings WHERE module = ?1 AND key = ?2",
                    [module, name],
                    |r| r.get(0),
                )
                .unwrap();
            let logged = c
                .query_row(
                    "SELECT COUNT(*) FROM change_log WHERE entity = 'setting' AND entity_id = ?1",
                    [key],
                    |r| r.get(0),
                )
                .unwrap();
            (rows, logged)
        })
    }

    /// Every look-and-feel key with a non-default value.
    fn look_cases() -> Vec<(&'static str, Value)> {
        vec![
            ("core.theme", json!("oled")),
            ("core.density", json!("compact")),
            ("core.text_size", json!("l")),
            ("core.reduce_motion", json!(true)),
            ("core.phone_tabs", json!(["home", "settings"])),
        ]
    }

    #[test]
    fn each_look_and_feel_key_saves_to_the_device_file_not_the_database() {
        for (key, value) in look_cases() {
            let app = TestApp::new();
            app.set_pref(key, value.clone());
            assert_eq!(get_all(&app)["values"][key], value, "{key} reads back");
            let short = key.strip_prefix("core.").unwrap();
            assert_eq!(
                device(&app)["look"][short],
                value,
                "{key} is in the device file"
            );
            assert_eq!(stored_rows(&app, key), (0, 0), "{key} is not a Preference");
        }
    }

    #[test]
    fn saving_phone_tabs_marks_them_edited_on_the_device() {
        let app = TestApp::new();
        assert_eq!(get_all(&app)["values"]["core.phone_tabs_edited"], false);
        app.set_pref("core.phone_tabs", json!(["home"]));
        assert_eq!(get_all(&app)["values"]["core.phone_tabs_edited"], true);
        assert_eq!(device(&app)["look"]["phone_tabs_edited"], true);
        assert_eq!(stored_rows(&app, "core.phone_tabs_edited"), (0, 0));
    }

    #[test]
    fn look_and_feel_is_validated_like_any_setting() {
        let app = TestApp::new();
        let err = save_theme(&app, json!("neon")).unwrap_err();
        assert_eq!(err["code"], "validation");
        assert_eq!(device(&app)["look"]["theme"], Value::Null);
    }

    #[test]
    fn a_batch_of_preferences_and_look_and_feel_saves_both_and_tells_once() {
        let app = TestApp::new();
        app.invoke::<()>(
            "settings_save",
            json!({ "changes": [
                { "key": "core.theme", "value": "dark" },
                { "key": "core.display_name", "value": "Hemang" },
            ] }),
        )
        .unwrap();
        let values = &get_all(&app)["values"];
        assert_eq!(values["core.theme"], "dark");
        assert_eq!(values["core.display_name"], "Hemang");
        assert_eq!(stored_rows(&app, "core.display_name"), (1, 1));
        assert_eq!(stored_rows(&app, "core.theme"), (0, 0));
    }

    #[test]
    fn an_upgrade_copies_existing_look_and_feel_and_deletes_the_rows() {
        let seed = |c: &Connection| {
            for (key, raw) in [
                ("theme", "\"oled\""),
                ("density", "\"compact\""),
                ("text_size", "\"l\""),
                ("reduce_motion", "true"),
                ("phone_tabs", "[\"home\",\"settings\"]"),
                ("phone_tabs_edited", "true"),
                ("display_name", "\"Hemang\""),
            ] {
                c.execute(
                    "INSERT INTO settings(module, key, value, updated_at) VALUES ('core', ?1, ?2, 0)",
                    [key, raw],
                )
                .unwrap();
            }
        };
        let app = TestApp::upgrading(seed, DeviceSettings::default());
        let values = &get_all(&app)["values"];
        assert_eq!(values["core.theme"], "oled");
        assert_eq!(values["core.density"], "compact");
        assert_eq!(values["core.text_size"], "l");
        assert_eq!(values["core.reduce_motion"], true);
        assert_eq!(values["core.phone_tabs"], json!(["home", "settings"]));
        assert_eq!(values["core.phone_tabs_edited"], true);
        for (key, _) in look_cases() {
            assert_eq!(stored_rows(&app, key).0, 0, "{key} row is gone");
        }
        assert_eq!(stored_rows(&app, "core.phone_tabs_edited").0, 0);
        assert_eq!(
            values["core.display_name"], "Hemang",
            "other Preferences stay"
        );
        assert_eq!(stored_rows(&app, "core.display_name").0, 1);
    }

    #[test]
    fn an_upgrade_never_overwrites_what_the_device_file_already_has() {
        let seed = |c: &Connection| {
            c.execute(
                "INSERT INTO settings(module, key, value, updated_at) VALUES
                 ('core', 'theme', '\"dark\"', 0), ('core', 'density', '\"compact\"', 0)",
                [],
            )
            .unwrap();
        };
        let device_file = DeviceSettings {
            look: LookAndFeel {
                theme: Some("light".into()),
                ..LookAndFeel::default()
            },
            ..DeviceSettings::default()
        };
        let app = TestApp::upgrading(seed, device_file);
        let values = &get_all(&app)["values"];
        assert_eq!(values["core.theme"], "light", "the device file wins");
        assert_eq!(values["core.density"], "compact", "the gap is filled");
        assert_eq!(stored_rows(&app, "core.theme").0, 0);
        assert_eq!(stored_rows(&app, "core.density").0, 0);
    }

    #[test]
    fn the_upgrade_is_safe_to_run_again() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        let report = db::migrate::run(&mut conn, &[&CoreModule], dir.path()).unwrap();
        conn.execute(
            "INSERT INTO settings(module, key, value, updated_at) VALUES ('core', 'theme', '\"dark\"', 0)",
            [],
        )
        .unwrap();
        let device = DeviceStore::new(dir.path().join("device.toml"), DeviceSettings::default());
        for _ in 0..2 {
            CoreModule.on_startup(&mut conn, &device, &report).unwrap();
        }
        assert_eq!(device.get().look.theme.as_deref(), Some("dark"));
        // A user's later choice survives another start.
        device
            .update(|d| d.look.theme = Some("light".into()))
            .unwrap();
        CoreModule.on_startup(&mut conn, &device, &report).unwrap();
        assert_eq!(device.get().look.theme.as_deref(), Some("light"));
    }

    #[test]
    fn a_corrupt_stored_look_value_reads_as_the_default() {
        let seed = |c: &Connection| {
            c.execute(
                "INSERT INTO settings(module, key, value, updated_at) VALUES ('core', 'theme', '\"neon\"', 0)",
                [],
            )
            .unwrap();
        };
        let app = TestApp::upgrading(seed, DeviceSettings::default());
        assert_eq!(get_all(&app)["values"]["core.theme"], "system");
    }

    #[test]
    fn saving_a_preference_leaves_a_setting_record() {
        let app = TestApp::new();
        app.set_pref("core.display_name", json!("Hemang"));
        let (id, snapshot): (String, String) = app.with_db(|c| {
            c.query_row(
                "SELECT entity_id, snapshot FROM change_log WHERE entity = 'setting'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
        });
        assert_eq!(id, "core.display_name");
        assert!(snapshot.contains("\"Hemang\""));
    }

    #[test]
    fn settings_get_all_returns_defaults_and_schema() {
        let app = TestApp::new();
        let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(snapshot["values"]["core.theme"], "system");
        assert!(!snapshot["schema"].as_array().unwrap().is_empty());
    }

    #[test]
    fn saved_setting_reads_back() {
        let app = TestApp::new();
        save_theme(&app, json!("dark")).unwrap();
        let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(snapshot["values"]["core.theme"], "dark");
    }

    #[test]
    fn invalid_setting_is_rejected_with_the_validation_code() {
        let app = TestApp::new();
        let err = save_theme(&app, json!("neon")).unwrap_err();
        assert_eq!(err["code"], "validation");
        let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(snapshot["values"]["core.theme"], "system");
    }

    #[test]
    fn unknown_setting_key_is_rejected() {
        let app = TestApp::new();
        let err = app
            .invoke::<()>(
                "settings_save",
                json!({ "changes": [{ "key": "core.nope", "value": 1 }] }),
            )
            .unwrap_err();
        assert_eq!(err["code"], "unknown_key");
    }

    #[test]
    fn spellcheck_is_on_by_default_and_round_trips() {
        let app = TestApp::new();
        let device: Value = app.invoke("device_get", json!({})).unwrap();
        assert_eq!(device["spellcheck"], true);
        app.invoke::<()>("device_set_spellcheck", json!({ "enabled": false }))
            .unwrap();
        let device: Value = app.invoke("device_get", json!({})).unwrap();
        assert_eq!(device["spellcheck"], false);
        app.invoke::<()>("device_set_spellcheck", json!({ "enabled": true }))
            .unwrap();
        let device: Value = app.invoke("device_get", json!({})).unwrap();
        assert_eq!(device["spellcheck"], true);
    }

    #[test]
    fn the_todo_fold_state_is_a_device_setting_with_no_choice_at_first() {
        let app = TestApp::new();
        let device: Value = app.invoke("device_get", json!({})).unwrap();
        assert_eq!(device["sidebar"]["todo_open"], Value::Null);
        app.invoke::<()>("device_set_todo_open", json!({ "open": true }))
            .unwrap();
        let device: Value = app.invoke("device_get", json!({})).unwrap();
        assert_eq!(device["sidebar"]["todo_open"], true);
        // The sidebar width and rail are saved separately and leave the fold state alone.
        app.invoke::<()>("device_set_sidebar", json!({ "width": 300, "rail": true }))
            .unwrap();
        let device: Value = app.invoke("device_get", json!({})).unwrap();
        assert_eq!(device["sidebar"]["todo_open"], true);
    }

    #[test]
    fn phone_tabs_default_to_home_journal_and_todo() {
        let app = TestApp::new();
        let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(
            snapshot["values"]["core.phone_tabs"],
            json!(["home", "journal", "todo"])
        );
    }

    /// An install's database and device file, which survive restarts.
    struct Install {
        conn: Connection,
        device: Arc<DeviceStore>,
        dir: tempfile::TempDir,
    }

    impl Install {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let device =
                DeviceStore::new(dir.path().join("device.toml"), DeviceSettings::default());
            Self {
                conn: db::open_in_memory().unwrap(),
                device: Arc::new(device),
                dir,
            }
        }

        /// Migrates like app start does with the given Modules, then runs their startup hooks.
        fn start(&mut self, modules: &[&dyn Module]) {
            let report = db::migrate::run(&mut self.conn, modules, self.dir.path()).unwrap();
            for m in modules {
                m.on_startup(&mut self.conn, &self.device, &report).unwrap();
            }
        }

        fn store(&self) -> mnemora_core::settings::SettingsStore {
            crate::settings_store(&crate::modules(), &EventBus::new(), &self.device)
        }

        /// The tabs as Settings would show them.
        fn tabs(&self) -> Value {
            self.store().get_all(&self.conn).unwrap().values["core.phone_tabs"].clone()
        }

        /// What the device file holds for the tabs, as a legacy or edited list.
        fn set_tabs_in_file(&self, value: &Value) {
            self.device
                .update(|d| {
                    d.look.set("phone_tabs", value);
                })
                .unwrap();
        }

        /// The user saves tabs in Settings ▸ Navigation.
        fn edit_tabs(&mut self, value: Value) {
            self.store()
                .save_batch(
                    &mut self.conn,
                    &[SettingsChange {
                        key: "core.phone_tabs".into(),
                        value,
                    }],
                )
                .unwrap();
        }
    }

    /// An install from before Todo: Core, and the Journal when `journal` (which pins Home only,
    /// as it does for an install that never chose).
    fn before_todo(journal: bool) -> Install {
        let mut install = Install::new();
        install.start(&[&CoreModule]);
        if journal {
            install.start(&[&CoreModule, &JournalModule]);
        }
        install
    }

    fn with_todo(install: &mut Install) {
        install.start(&[&CoreModule, &JournalModule, &TodoModule]);
    }

    #[test]
    fn an_upgrade_from_before_both_modules_gets_home_and_todo() {
        // The Journal's upgrade pins Home only (an install that never chose has no Journal tab);
        // the pin is not an edit, so Todo is appended to it.
        let mut install = before_todo(false);
        install.start(&[&CoreModule, &JournalModule, &TodoModule]);
        assert_eq!(install.tabs(), json!(["home", "todo"]));
    }

    #[test]
    fn the_journals_pin_gets_todo_appended_once() {
        let mut install = before_todo(true);
        assert_eq!(install.tabs(), json!(["home"]));
        with_todo(&mut install);
        assert_eq!(install.tabs(), json!(["home", "todo"]));
        // Later starts change nothing.
        with_todo(&mut install);
        assert_eq!(install.tabs(), json!(["home", "todo"]));
    }

    #[test]
    fn removing_todo_afterwards_sticks() {
        let mut install = before_todo(true);
        with_todo(&mut install);
        install.edit_tabs(json!(["home"]));
        with_todo(&mut install);
        assert_eq!(install.tabs(), json!(["home"]));
    }

    #[test]
    fn tabs_the_user_edited_are_left_alone() {
        for edited in [json!(["home", "settings"]), json!(["home"])] {
            let mut install = before_todo(true);
            install.edit_tabs(edited.clone());
            with_todo(&mut install);
            assert_eq!(install.tabs(), edited);
        }
    }

    #[test]
    fn a_legacy_edited_list_without_the_marker_is_left_alone() {
        // Saved tabs other than exactly Home or Home, Journal count as edited even when the
        // `phone_tabs_edited` marker is missing (an install that predates it).
        for legacy in [
            json!(["home", "settings"]),
            json!(["journal", "home"]),
            json!(["home", "journal", "settings"]),
            json!(["settings"]),
        ] {
            let mut install = before_todo(true);
            install.set_tabs_in_file(&legacy);
            with_todo(&mut install);
            assert_eq!(install.tabs(), legacy);
        }
    }

    #[test]
    fn the_unedited_home_journal_pair_gets_todo() {
        let mut install = before_todo(true);
        install.set_tabs_in_file(&json!(["home", "journal"]));
        with_todo(&mut install);
        assert_eq!(install.tabs(), json!(["home", "journal", "todo"]));
    }

    #[test]
    fn four_tabs_get_no_fifth() {
        let mut install = before_todo(true);
        let four = json!(["home", "journal", "settings", "a"]);
        install.set_tabs_in_file(&four);
        with_todo(&mut install);
        assert_eq!(install.tabs(), four);
    }

    #[test]
    fn todo_switched_off_is_not_added() {
        let mut install = before_todo(true);
        install
            .conn
            .execute(
                "INSERT INTO settings(module, key, value, updated_at) VALUES ('core', 'disabled_modules', '[\"todo\"]', 0)",
                [],
            )
            .unwrap();
        with_todo(&mut install);
        assert_eq!(install.tabs(), json!(["home"]));
    }

    #[test]
    fn an_install_that_never_saved_its_tabs_reads_the_new_default() {
        let mut install = before_todo(false);
        with_todo(&mut install);
        // The Journal arrived in the same start and pinned Home; Todo was appended to it.
        assert_eq!(install.tabs(), json!(["home", "todo"]));
        let app = TestApp::new();
        let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
        assert_eq!(
            snapshot["values"]["core.phone_tabs"],
            json!(["home", "journal", "todo"])
        );
    }

    #[test]
    fn tabs_saved_before_the_move_survive_the_upgrade_and_still_count_as_edited() {
        // An old database with saved tabs and the edited marker in the table, then a restart on
        // this version: the move runs before the Modules' upgrade steps look at the tabs.
        let mut install = before_todo(true);
        for (key, raw) in [("phone_tabs", "[\"home\"]"), ("phone_tabs_edited", "true")] {
            install
                .conn
                .execute(
                    "INSERT OR REPLACE INTO settings(module, key, value, updated_at) VALUES ('core', ?1, ?2, 0)",
                    [key, raw],
                )
                .unwrap();
        }
        install
            .device
            .update(|d| d.look = LookAndFeel::default())
            .unwrap();
        with_todo(&mut install);
        assert_eq!(install.tabs(), json!(["home"]));
    }

    #[test]
    fn keeps_a_real_first_name() {
        assert_eq!(
            sanitize_first_name("  Hemang Khatri"),
            Some("Hemang".into())
        );
    }

    #[test]
    fn root_and_user_may_be_real_names() {
        assert_eq!(sanitize_first_name("root"), Some("root".into()));
        assert_eq!(sanitize_first_name("User"), Some("User".into()));
    }

    #[test]
    fn placeholders_and_blanks_are_no_name() {
        for n in ["Anonymous", "UNKNOWN", "", "   "] {
            assert_eq!(sanitize_first_name(n), None, "{n:?}");
        }
    }
}
