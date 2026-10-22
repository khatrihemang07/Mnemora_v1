//! Command-layer test harness: the app's real commands, driven through Tauri's mock runtime.
//!
//! ```ignore
//! let app = TestApp::new();
//! app.invoke::<()>("settings_save", json!({ "changes": [{ "key": "core.theme", "value": "dark" }] }))
//!     .unwrap();
//! let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
//! ```

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use mnemora_core::db::migrate::MigrationReport;
use mnemora_core::db::{self, Connection};
use mnemora_core::device::{DeviceSettings, DeviceStore};
use mnemora_core::env::Environment;
use mnemora_core::events::EventBus;
use mnemora_core::logging::LogHandle;
use mnemora_core::module::Module;
use mnemora_core::paths::AppPaths;
use mnemora_core::util::now_ms;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tauri::http::HeaderMap;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::InvokeRequest;
use tauri::{Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tempfile::TempDir;

use crate::state::{AppState, lock};
use crate::widget::signal::WidgetSignal;
use crate::{
    LinkOpenedEvent, SettingsChangedEvent, TodoChangedEvent, commands, journal, links, modules,
    settings_store, todo,
};

/// The wall clock a [`TestApp`] stamps its change-log records with: the system clock plus a
/// skew, or frozen at a chosen instant. Lets a test (and later a cluster of `TestApp`s) make one
/// device's clock run ahead, stand still or lag behind another's.
#[derive(Default)]
pub struct TestClock {
    /// Milliseconds added to the system clock.
    skew_ms: AtomicI64,
    /// When non-zero, the clock stands still at this instant.
    frozen_at: AtomicI64,
}

impl TestClock {
    fn now(&self) -> i64 {
        match self.frozen_at.load(Ordering::SeqCst) {
            0 => now_ms() + self.skew_ms.load(Ordering::SeqCst),
            frozen => frozen,
        }
    }

    /// Stands still at `ms`.
    pub fn freeze(&self, ms: i64) {
        self.frozen_at.store(ms, Ordering::SeqCst);
    }

    /// Moves a frozen clock forward by `ms` (or a running one's skew).
    pub fn advance(&self, ms: i64) {
        if self.frozen_at.load(Ordering::SeqCst) == 0 {
            self.skew_ms.fetch_add(ms, Ordering::SeqCst);
        } else {
            self.frozen_at.fetch_add(ms, Ordering::SeqCst);
        }
    }

    /// Runs `ms` ahead of (or, negative, behind) the system clock, and keeps running.
    pub fn skew(&self, ms: i64) {
        self.frozen_at.store(0, Ordering::SeqCst);
        self.skew_ms.store(ms, Ordering::SeqCst);
    }
}

/// A mock Tauri app with every registered command, on an in-memory database migrated with every
/// Module. Device files and backups go to a temp folder that is removed on drop.
pub struct TestApp {
    // Field order matters: the app (and its database) drops before the folder.
    window: WebviewWindow<MockRuntime>,
    app: tauri::App<MockRuntime>,
    clock: Arc<TestClock>,
    _dir: TempDir,
}

impl TestApp {
    /// Builds the app. Panics on failure; this is test setup.
    pub fn new() -> Self {
        Self::upgrading(|_| {}, DeviceSettings::default())
    }

    /// Like [`new`](Self::new), for an install that already has data: `seed` writes to the
    /// freshly migrated database and `device` is what `device.toml` held, then the Modules'
    /// startup work runs as it does when the app starts.
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    pub fn upgrading(seed: impl FnOnce(&db::Connection), device: DeviceSettings) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = AppPaths::resolve(Environment::Production, dir.path(), None);

        let modules = modules();
        let module_refs: Vec<&dyn Module> = modules.iter().map(AsRef::as_ref).collect();
        let mut conn = db::open_in_memory().unwrap();
        let clock = Arc::new(TestClock::default());
        let source = Arc::clone(&clock);
        db::set_time_source(&conn, Arc::new(move || source.now())).unwrap();
        let report = db::migrate::run(&mut conn, &module_refs, &paths.backup_dir).unwrap();
        seed(&conn);
        let device = Arc::new(DeviceStore::new(paths.device_file(), device));
        for module in &modules {
            module.on_startup(&mut conn, &device, &report).unwrap();
        }

        let bus = EventBus::new();
        let widgets = Arc::new(WidgetSignal::silent());
        crate::widget::signal::follow_settings(&bus, &widgets);
        let state = AppState {
            env: Environment::Production,
            db: Mutex::new(conn),
            settings: settings_store(&modules, &bus, &device),
            device,
            paths,
            log: LogHandle::detached(),
            widgets,
        };

        let app = mock_builder()
            .invoke_handler(specta_builder_for!(MockRuntime).invoke_handler())
            .manage(state)
            .manage(crate::links::PendingLinks::default())
            .build(mock_context(noop_assets()))
            .unwrap();
        // Typed events can only be emitted once mounted, as in the real app.
        specta_builder_for!(MockRuntime).mount_events(&app);
        let window = WebviewWindowBuilder::new(&app, "main", WebviewUrl::default())
            .build()
            .unwrap();
        Self {
            window,
            app,
            clock,
            _dir: dir,
        }
    }

    /// Invokes `command` (`snake_case` name) with JSON `args` keyed by parameter name, as the UI
    /// would. `Ok` is the response deserialised into `T`; `Err` is the serialised `AppError`
    /// (`{code, message, details}`).
    #[allow(clippy::unwrap_used)]
    pub fn invoke<T: DeserializeOwned>(&self, command: &str, args: Value) -> Result<T, Value> {
        let request = InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: if cfg!(any(windows, target_os = "android")) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .unwrap(),
            body: InvokeBody::Json(args),
            headers: HeaderMap::default(),
            invoke_key: INVOKE_KEY.to_string(),
        };
        get_ipc_response(&self.window, request).map(|body| body.deserialize().unwrap())
    }

    /// Saves one Setting, as the Settings screen would: `key` is the full `module.key`.
    #[allow(clippy::unwrap_used)]
    pub fn set_pref(&self, key: &str, value: Value) {
        self.invoke::<()>(
            "settings_save",
            serde_json::json!({ "changes": [{ "key": key, "value": value }] }),
        )
        .unwrap();
    }

    /// The app's handle: what the Todo widget's bridge uses when the app is alive.
    pub fn handle(&self) -> tauri::AppHandle<MockRuntime> {
        self.app.handle().clone()
    }

    /// How many times the Todo widget has been told that Todo changed.
    pub fn widget_signals(&self) -> u32 {
        self.app.state::<AppState>().widgets.sent()
    }

    /// This app's wall clock: freeze, advance or skew it to simulate a device whose time differs.
    pub fn clock(&self) -> &TestClock {
        &self.clock
    }

    /// This install's device id, as `device.toml` holds it.
    pub fn device_id(&self) -> String {
        self.app.state::<AppState>().device.device_id()
    }

    /// Runs `f` on the database, for assertions about state no command exposes (the change log).
    pub fn with_db<R>(&self, f: impl FnOnce(&Connection) -> R) -> R {
        f(&lock(&self.app.state::<AppState>().db))
    }

    /// Runs a Module's startup work again, as an app restart would (nothing newly migrated).
    #[allow(clippy::unwrap_used)]
    pub fn run_startup(&self, module: &dyn Module) {
        let state = self.app.state::<AppState>();
        module
            .on_startup(
                &mut lock(&state.db),
                &state.device,
                &MigrationReport::default(),
            )
            .unwrap();
    }
}

/// The Tasks of a grouped view's `groups`, flattened in order. Overdue is left out (see
/// `overdue_of`), so for Today this is what is due today.
pub fn tasks_of(view: &Value) -> Value {
    flatten(view, false)
}

/// The Tasks in a grouped view's Overdue group.
pub fn overdue_of(view: &Value) -> Value {
    flatten(view, true)
}

/// The Tasks of an Inbox view that count toward the Inbox: groups that are not "No date, in a
/// List".
pub fn inbox_tasks_of(view: &Value) -> Value {
    let tasks = view["groups"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|g| g["key"]["kind"] != "no_date_in_list")
        .flat_map(|g| g["tasks"].as_array().cloned().unwrap_or_default());
    Value::Array(tasks.collect())
}

fn flatten(view: &Value, overdue: bool) -> Value {
    let tasks = view["groups"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|g| (g["key"]["kind"] == "overdue") == overdue)
        .flat_map(|g| g["tasks"].as_array().cloned().unwrap_or_default());
    Value::Array(tasks.collect())
}

/// Stamps every closed Task as closed at noon UTC on `date`. Completing stamps the real clock,
/// which is never inside a pinned test day, so tests that read the Done window pin it here.
pub fn close_all_on(app: &TestApp, date: &str) {
    app.with_db(|c| {
        c.execute(
            "UPDATE todo_tasks SET closed_at = CAST(strftime('%s', ?1 || ' 12:00:00') AS INTEGER) * 1000
             WHERE state <> 'open'",
            [date],
        )
        .unwrap();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_app_is_its_own_device_and_keeps_its_id_across_restarts() {
        let a = TestApp::new();
        let b = TestApp::new();
        assert_ne!(a.device_id(), b.device_id());
        let saved = DeviceSettings {
            device_id: Some(a.device_id()),
            ..DeviceSettings::default()
        };
        let restarted = TestApp::upgrading(|_| {}, saved);
        assert_eq!(restarted.device_id(), a.device_id());
    }

    #[test]
    fn records_are_stamped_with_the_apps_device_and_its_controllable_clock() {
        let app = TestApp::new();
        app.clock().freeze(1_800_000_000_000);
        app.invoke::<Value>("todo_list_create", serde_json::json!({ "name": "Work" }))
            .unwrap();
        app.clock().advance(5_000);
        app.invoke::<Value>("todo_list_create", serde_json::json!({ "name": "Home" }))
            .unwrap();
        let rows: Vec<(String, i64)> = app.with_db(|c| {
            c.prepare("SELECT origin_device, hlc_ms FROM change_log ORDER BY seq")
                .unwrap()
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        });
        assert_eq!(
            rows,
            vec![
                (app.device_id(), 1_800_000_000_000),
                (app.device_id(), 1_800_000_005_000)
            ]
        );
    }

    #[test]
    fn a_skewed_clock_runs_ahead_of_the_system_clock() {
        let app = TestApp::new();
        app.clock().skew(3_600_000);
        app.invoke::<Value>("todo_list_create", serde_json::json!({ "name": "Work" }))
            .unwrap();
        let ms: i64 = app.with_db(|c| {
            c.query_row("SELECT hlc_ms FROM change_log", [], |r| r.get(0))
                .unwrap()
        });
        assert!(ms >= now_ms() + 3_500_000, "an hour ahead: {ms}");
    }
}
