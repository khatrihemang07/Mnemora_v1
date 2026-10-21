//! Mnemora app shell: wires `mnemora-core` into Tauri. Logic belongs in core, not here.

// `#[tauri::mobile_entry_point]` expands to an `extern "C"` export that trips `unsafe_code`.
#![cfg_attr(mobile, allow(unsafe_code))]

// Defined before the modules so the test harness can reuse it.
/// Every command and event for one concrete runtime: `Wry` in the app, the mock in tests.
/// A macro because `#[specta::specta]` cannot see a generic runtime parameter of an outer fn.
macro_rules! specta_builder_for {
    ($runtime:ty) => {
        tauri_specta::Builder::<$runtime>::new()
            .disable_serde_phases()
            .commands(tauri_specta::collect_commands![
                commands::settings_get_all,
                commands::settings_save,
                commands::device_get,
                commands::device_set_sidebar,
                commands::device_set_todo_open,
                commands::device_set_log_level,
                commands::device_set_spellcheck,
                commands::app_info,
                commands::log_ui_error,
                commands::backup_now,
                commands::open_log_dir::<$runtime>,
                journal::journal_entry_create,
                journal::journal_entry_update,
                journal::journal_entry_toggle_task,
                journal::journal_entry_delete,
                journal::journal_entry_restore,
                journal::journal_entry_move,
                journal::journal_month_days,
                journal::journal_pages,
                journal::journal_search,
                journal::journal_today,
                todo::todo_inbox,
                todo::todo_task_create,
                todo::todo_parse,
                todo::todo_task_quick_edit,
                todo::todo_task_quick_edit_undo,
                todo::todo_task_complete,
                todo::todo_task_reopen,
                todo::todo_task_skip,
                todo::todo_done,
                todo::todo_task_move,
                todo::todo_task,
                todo::todo_search,
                todo::todo_task_update,
                todo::todo_task_set_priority,
                todo::todo_task_wont_do,
                todo::todo_task_delete,
                todo::todo_task_restore,
                todo::todo_comments,
                todo::todo_comment_add,
                todo::todo_comment_edit,
                todo::todo_comment_delete,
                todo::todo_comment_restore,
                todo::todo_task_history,
                todo::todo_task_set_when,
                todo::todo_tasks_restore_when,
                todo::todo_tasks_move_overdue,
                todo::todo_today,
                todo::todo_upcoming,
                todo::todo_day_counts,
                todo::todo_counts,
                todo::todo_lists,
                todo::todo_list,
                todo::todo_all,
                todo::todo_list_create,
                todo::todo_list_rename,
                todo::todo_list_set_emoji,
                todo::todo_list_delete,
                todo::todo_list_restore,
                todo::todo_task_reorder,
                todo::todo_list_reorder,
                todo::todo_list_set_sort,
                todo::todo_list_set_group,
            ])
            .events(tauri_specta::collect_events![
                SettingsChangedEvent,
                TodoChangedEvent
            ])
    };
}

mod commands;
mod env_guard;
mod journal;
mod state;
#[cfg(test)]
mod test_support;
mod theme_inject;
mod todo;
#[cfg(test)]
mod todo_capture_tests;
#[cfg(test)]
mod todo_order_tests;
#[cfg(test)]
mod todo_search_tests;
#[cfg(test)]
mod todo_sheet_tests;
mod widget;
#[cfg(test)]
mod widget_tests;
#[cfg(desktop)]
mod window_state;

use std::error::Error;
use std::sync::{Arc, Mutex};

use mnemora_core::db;
use mnemora_core::device::DeviceSettings;
use mnemora_core::env::Environment;
use mnemora_core::events::{EventBus, SettingsChanged};
use mnemora_core::logging;
use mnemora_core::module::{CoreModule, Module};
use mnemora_core::paths::AppPaths;
use mnemora_core::settings::{SettingsRegistry, SettingsStore};
use mnemora_journal::JournalModule;
use mnemora_todo::TodoModule;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{Manager, WebviewWindowBuilder};
use tauri_specta::{Builder, Event};

use crate::state::{AppState, DeviceStore};

/// Decided at build time; there is deliberately no runtime switch that could mix data sets.
const ENVIRONMENT: Environment = if cfg!(feature = "sandbox") {
    Environment::Sandbox
} else {
    Environment::Production
};

/// Where the generated TypeScript bindings go (consumed by the UI).
#[cfg(all(debug_assertions, desktop))]
const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/ipc/bindings.ts");

/// Typed IPC event mirroring core's `SettingsChanged`, emitted to the webview.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct SettingsChangedEvent(pub SettingsChanged);

/// Todo data was changed by something other than the app's own screens (the Todo widget), so
/// whatever Todo shows is stale.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct TodoChangedEvent;

/// Every Module in the app, in migration order. The one place Modules are registered.
fn modules() -> Vec<Box<dyn Module>> {
    vec![
        Box::new(CoreModule),
        Box::new(JournalModule),
        Box::new(TodoModule),
    ]
}

/// A Settings store over every Module's registered keys.
fn settings_store(modules: &[Box<dyn Module>], bus: &EventBus) -> SettingsStore {
    let mut registry = SettingsRegistry::new();
    for module in modules {
        module.register_settings(&mut registry);
    }
    SettingsStore::new(Arc::new(registry), bus.clone())
}

/// Commands and events exposed over IPC. Also used to export bindings.
pub fn specta_builder() -> Builder<tauri::Wry> {
    specta_builder_for!(tauri::Wry)
}

/// Writes `src/lib/ipc/bindings.ts`.
///
/// # Errors
/// Fails if the file cannot be written.
#[cfg(desktop)]
pub fn export_bindings(path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    specta_builder().export(specta_typescript::Typescript::default(), path)?;
    Ok(())
}

fn setup(app: &mut tauri::App, builder: &Builder<tauri::Wry>) -> Result<(), Box<dyn Error>> {
    let base = app.path().data_dir()?;
    env_guard::check(
        ENVIRONMENT,
        &app.config().identifier,
        &base.to_string_lossy(),
        cfg!(target_os = "android"),
    )?;

    // Logging first so everything after it (including a corrupt device.toml) is recorded.
    let boot_paths = AppPaths::resolve(ENVIRONMENT, &base, None);
    let log = logging::init(&boot_paths.log_dir, "info")?;
    tracing::info!(
        env = ENVIRONMENT.as_str(),
        version = env!("CARGO_PKG_VERSION"),
        "starting"
    );

    let device = DeviceSettings::load(&boot_paths.device_file());
    if let Err(e) = log.set_level(&device.log_level) {
        tracing::warn!(error = %e, "ignoring invalid saved log level");
    }
    let paths = AppPaths::resolve(ENVIRONMENT, &base, device.data_dir_override().as_deref());

    let modules = modules();
    let module_refs: Vec<&dyn Module> = modules.iter().map(AsRef::as_ref).collect();
    let mut conn = db::open(&paths.db_file)?;
    let report = db::migrate::run(&mut conn, &module_refs, &paths.backup_dir)?;
    tracing::info!(applied = ?report.applied, backup = ?report.backup, "database ready");

    // Once per start, after migrations. A failing Module must not stop the app.
    for module in &modules {
        if let Err(e) = module.on_startup(&mut conn, &report) {
            tracing::warn!(module = module.id(), error = %e, "startup work failed");
        }
    }

    let bus = EventBus::new();
    let settings = settings_store(&modules, &bus);
    let snapshot = settings.get_all(&conn)?;

    // Bridge core's in-process event to a typed IPC event.
    let handle = app.handle().clone();
    bus.subscribe(move |event: &SettingsChanged| {
        if let Err(e) = SettingsChangedEvent(event.clone()).emit(&handle) {
            tracing::warn!(error = %e, "could not emit settings event");
        }
    });
    builder.mount_events(app);

    let state = AppState {
        env: ENVIRONMENT,
        db: Mutex::new(conn),
        settings,
        device: Arc::new(DeviceStore::new(paths.device_file(), device.clone())),
        paths,
        log,
        widgets: widget::signal::WidgetSignal::platform(),
    };
    app.manage(state);
    #[cfg(target_os = "android")]
    widget::register_app(app.handle().clone());
    // A Todo widget placed before the app ever ran (or before an update) shows a "open Mnemora"
    // state; now that the database exists and is current, it should redraw without waiting for
    // the next Todo write.
    app.state::<AppState>().widgets.todo_changed();

    create_main_window(app, &snapshot.values, &device)?;
    Ok(())
}

fn create_main_window(
    app: &tauri::App,
    values: &std::collections::BTreeMap<String, serde_json::Value>,
    device: &DeviceSettings,
) -> Result<(), Box<dyn Error>> {
    let config = app
        .config()
        .app
        .windows
        .first()
        .ok_or("no window configured in tauri.conf.json")?
        .clone();
    let builder = WebviewWindowBuilder::from_config(app.handle(), &config)?
        .initialization_script(theme_inject::script(ENVIRONMENT, values));
    #[cfg(desktop)]
    let builder = window_state::restore(app.handle(), builder, device);
    #[cfg(not(desktop))]
    let _ = device;
    let window = builder.build()?;
    #[cfg(desktop)]
    window_state::track(&window, Arc::clone(&app.state::<AppState>().device));
    #[cfg(not(desktop))]
    let _ = window;
    Ok(())
}

/// App entry point (desktop `main` and the mobile library entry).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    #[cfg(all(debug_assertions, desktop))]
    if let Err(e) = export_bindings(std::path::Path::new(BINDINGS_PATH)) {
        // Runs before the tracing subscriber exists (it is set up in `setup`), so stderr it is.
        eprintln!("could not export TypeScript bindings: {e}");
    }

    let invoke_handler = builder.invoke_handler();
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(invoke_handler)
        .setup(move |app| setup(app, &builder))
        .run(tauri::generate_context!());
    if let Err(e) = result {
        tracing::error!(error = %e, "fatal");
        // Setup can fail before `logging::init` (e.g. environment mismatch), when tracing
        // has no subscriber and would print nothing; keep stderr as the fallback.
        eprintln!("fatal: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_expose_every_command_and_event() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("bindings.ts");
        export_bindings(&out).unwrap();
        let ts = std::fs::read_to_string(out).unwrap();
        for name in [
            "settingsGetAll",
            "settingsSave",
            "deviceGet",
            "deviceSetSidebar",
            "deviceSetTodoOpen",
            "deviceSetLogLevel",
            "appInfo",
            "logUiError",
            "backupNow",
            "openLogDir",
            "journalEntryCreate",
            "journalEntryUpdate",
            "journalEntryToggleTask",
            "journalEntryDelete",
            "journalEntryRestore",
            "journalEntryMove",
            "journalMonthDays",
            "journalPages",
            "journalSearch",
            "journalToday",
            "todoInbox",
            "todoTaskCreate",
            "todoParse",
            "todoTaskQuickEdit",
            "todoTaskQuickEditUndo",
            "todoTaskComplete",
            "todoTaskReopen",
            "todoTaskSkip",
            "todoDone",
            "todoTaskMove",
            "todoTask",
            "todoSearch",
            "todoTaskUpdate",
            "todoTaskSetPriority",
            "todoTaskWontDo",
            "todoTaskDelete",
            "todoTaskRestore",
            "todoComments",
            "todoCommentAdd",
            "todoCommentEdit",
            "todoCommentDelete",
            "todoCommentRestore",
            "todoTaskHistory",
            "todoTaskSetWhen",
            "todoTasksRestoreWhen",
            "todoTasksMoveOverdue",
            "todoToday",
            "todoUpcoming",
            "todoDayCounts",
            "todoCounts",
            "todoLists",
            "todoList",
            "todoAll",
            "todoListCreate",
            "todoListRename",
            "todoListSetEmoji",
            "todoListDelete",
            "todoListRestore",
            "todoTaskReorder",
            "todoListReorder",
            "todoListSetSort",
            "todoListSetGroup",
            "deviceSetSpellcheck",
            "settingsChangedEvent",
        ] {
            assert!(ts.contains(name), "bindings are missing {name}");
        }
    }

    /// Regenerates the checked-in bindings without launching the GUI:
    /// `cargo test -p mnemora-app regenerate_bindings -- --ignored` (or `make bindings`).
    #[test]
    #[ignore = "writes into the source tree"]
    fn regenerate_bindings() {
        export_bindings(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../src/lib/ipc/bindings.ts"
        )))
        .unwrap();
    }
}
