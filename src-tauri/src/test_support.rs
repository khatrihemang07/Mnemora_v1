//! Command-layer test harness: the app's real commands, driven through Tauri's mock runtime.
//!
//! ```ignore
//! let app = TestApp::new();
//! app.invoke::<()>("settings_save", json!({ "changes": [{ "key": "core.theme", "value": "dark" }] }))
//!     .unwrap();
//! let snapshot: Value = app.invoke("settings_get_all", json!({})).unwrap();
//! ```

use std::sync::{Arc, Mutex};

use mnemora_core::db::{self, Connection};
use mnemora_core::device::DeviceSettings;
use mnemora_core::env::Environment;
use mnemora_core::events::EventBus;
use mnemora_core::logging::LogHandle;
use mnemora_core::module::Module;
use mnemora_core::paths::AppPaths;
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

use crate::state::{AppState, DeviceStore, lock};
use crate::widget::signal::WidgetSignal;
use crate::{
    LinkOpenedEvent, SettingsChangedEvent, TodoChangedEvent, commands, journal, links, modules,
    settings_store, todo,
};

/// A mock Tauri app with every registered command, on an in-memory database migrated with every
/// Module. Device files and backups go to a temp folder that is removed on drop.
pub struct TestApp {
    // Field order matters: the app (and its database) drops before the folder.
    window: WebviewWindow<MockRuntime>,
    app: tauri::App<MockRuntime>,
    _dir: TempDir,
}

impl TestApp {
    /// Builds the app. Panics on failure; this is test setup.
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let paths = AppPaths::resolve(Environment::Production, dir.path(), None);

        let modules = modules();
        let module_refs: Vec<&dyn Module> = modules.iter().map(AsRef::as_ref).collect();
        let mut conn = db::open_in_memory().unwrap();
        db::migrate::run(&mut conn, &module_refs, &paths.backup_dir).unwrap();

        let bus = EventBus::new();
        let widgets = Arc::new(WidgetSignal::silent());
        crate::widget::signal::follow_settings(&bus, &widgets);
        let state = AppState {
            env: Environment::Production,
            db: Mutex::new(conn),
            settings: settings_store(&modules, &bus),
            device: Arc::new(DeviceStore::new(
                paths.device_file(),
                DeviceSettings::default(),
            )),
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

    /// Runs `f` on the database, for assertions about state no command exposes (the change log).
    pub fn with_db<R>(&self, f: impl FnOnce(&Connection) -> R) -> R {
        f(&lock(&self.app.state::<AppState>().db))
    }

    /// Like `with_db`, for code that needs `&mut Connection` (it opens its own transaction).
    pub fn with_db_mut<R>(&self, f: impl FnOnce(&mut Connection) -> R) -> R {
        f(&mut lock(&self.app.state::<AppState>().db))
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
