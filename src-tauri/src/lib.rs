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
                links::app_take_links,
                commands::log_ui_error,
                commands::backup_now,
                commands::open_log_dir::<$runtime>,
                editing::editing_hold,
                editing::editing_release,
                journal::journal_entry_create,
                journal::journal_entry_update,
                journal::journal_entry_toggle_task,
                ai::ai_run_start::<$runtime>,
                ai::ai_run_steer::<$runtime>,
                ai::ai_run_unqueue::<$runtime>,
                ai::ai_run_send_now::<$runtime>,
                ai::ai_run_abort,
                ai::ai_run_retry::<$runtime>,
                ai::ai_conversation_set_leaf,
                ai::ai_conversation_show_branch,
                ai::ai_conversation_get,
                ai::ai_provider_list,
                ai::ai_provider_create,
                ai::ai_provider_update,
                ai::ai_provider_delete,
                ai::ai_provider_key_set,
                ai::ai_provider_key_clear,
                ai::ai_provider_test,
                ai::ai_models_search,
                ai::ai_models_usage,
                ai::ai_conversation_list,
                ai::ai_conversation_rename,
                ai::ai_conversation_delete,
                ai::ai_conversation_restore,
                ai::ai_conversation_markdown,
                ai::ai_saved_list,
                ai::ai_saved_create,
                ai::ai_saved_update,
                ai::ai_saved_delete,
                ai::ai_saved_restore,
                ai::ai_saved_reorder,
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
                sync::sync_get,
                sync::sync_set_enabled::<$runtime>,
                sync::sync_rename,
                sync::sync_set_option::<$runtime>,
                sync::sync_leave,
                sync::sync_devices,
                sync::sync_device,
                sync::sync_device_rename,
                sync::sync_device_address,
                sync::sync_device_check,
                sync::sync_device_sync_now,
                sync::sync_device_forget,
                sync::sync_pair_state,
                sync::sync_pair_show,
                sync::sync_pair_new_code,
                sync::sync_pair_enter,
                sync::sync_pair_enter_at,
                sync::sync_pair_decide,
                sync::sync_pair_merge,
                sync::sync_pair_cancel,
                sync::sync_now,
                sync::sync_resume,
                sync::sync_open_local_network_settings::<$runtime>,
                sync::sync_local_network_os::<$runtime>,
                sync_conflicts::sync_conflicts,
                sync_conflicts::sync_conflict,
                sync_conflicts::sync_conflict_draft,
                sync_conflicts::sync_conflict_resolve,
                sync_conflicts::sync_attention,
                sync_conflicts::sync_activity,
                sync_conflicts::sync_resolution_use_other,
                sync_conflicts::sync_resolutions_visited,
            ])
            .events(tauri_specta::collect_events![
                SettingsChangedEvent,
                TodoChangedEvent,
                JournalChangedEvent,
                AiChangedEvent,
                LinkOpenedEvent,
                SyncPairingEvent,
                SyncPeersEvent,
                SyncConflictsEvent,
                SyncActivityEvent,
                SyncNewConflictsEvent,
                TodoListsMergedEvent,
                TodoTaskDeletedEvent,
                JournalEntryDeletedEvent,
                AiRunEvent
            ])
    };
}

mod ai;
#[cfg(test)]
mod ai_branching_tests;
#[cfg(test)]
mod ai_changes_tests;
#[cfg(test)]
mod ai_comment_hints_tests;
#[cfg(test)]
mod ai_conversations_tests;
#[cfg(any(test, feature = "seed"))]
mod ai_fixture;
#[cfg(test)]
mod ai_fixture_tests;
#[cfg(test)]
mod ai_provider_tests;
// The headless QA harness: real Runs over a copy of the Sandbox, for `examples/ai_qa.rs`.
#[cfg(any(test, feature = "seed"))]
#[doc(hidden)]
pub mod ai_qa;
#[cfg(test)]
mod ai_qa_tests;
#[cfg(test)]
mod ai_run_control_tests;
#[cfg(test)]
mod ai_saved_tests;
// The Sandbox seed: only in tests and the `seed` feature, never in the app's own surface.
#[cfg(any(test, feature = "seed"))]
#[doc(hidden)]
pub mod ai_seed;
#[cfg(test)]
mod ai_seed_tests;
#[cfg(test)]
mod ai_sync_tests;
#[cfg(test)]
mod ai_tools_tests;
#[cfg(test)]
mod ai_tracer_tests;
#[cfg(test)]
mod ai_wire_tests;
mod commands;
mod editing;
mod env_guard;
mod journal;
#[cfg(test)]
mod legacy_db_tests;
mod links;
mod state;
mod sync;
#[cfg(test)]
mod sync_activity_tests;
#[cfg(test)]
mod sync_address_tests;
#[cfg(test)]
mod sync_backfill_tests;
#[cfg(test)]
mod sync_cluster_tests;
mod sync_conflicts;
#[cfg(test)]
mod sync_conflicts_tests;
#[cfg(test)]
mod sync_forget_tests;
#[cfg(test)]
mod sync_groups_tests;
#[cfg(test)]
mod sync_journal_tests;
#[cfg(test)]
mod sync_lists_tests;
#[cfg(test)]
mod sync_live_tests;
#[cfg(test)]
mod sync_merge_fixture_tests;
#[cfg(test)]
mod sync_merge_preview_tests;
#[cfg(test)]
mod sync_pairing_tests;
#[cfg(test)]
mod sync_pause_tests;
mod sync_power;
#[cfg(test)]
mod sync_power_tests;
#[cfg(test)]
mod sync_resolutions_tests;
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

use mnemora_ai::AiModule;
use mnemora_core::db;
use mnemora_core::device::{DeviceSettings, DeviceStore};
use mnemora_core::env::Environment;
use mnemora_core::events::{
    ConflictsRaised, DeviceRenamed, EntryDeletedRemotely, EventBus, ListsMerged, RemoteChanged,
    RemoteSessionEnded, SettingsChanged, TaskDeletedRemotely,
};
use mnemora_core::logging;
use mnemora_core::module::{CoreModule, Module};
use mnemora_core::paths::AppPaths;
use mnemora_core::settings::{SettingsRegistry, SettingsStore};
use mnemora_journal::JournalModule;
use mnemora_sync::{Modules, SyncModule, SyncRuntime};
use mnemora_todo::TodoModule;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{Manager, WebviewWindowBuilder};
use tauri_specta::{Builder, Event};

use crate::state::AppState;

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

/// Sync changed Journal data from another device, so whatever the Journal shows is stale. The
/// UI refreshes in place, silently.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct JournalChangedEvent;

/// Sync changed AI data (Conversations, entries or Saved questions) from another device, so whatever
/// the AI page shows is stale. The UI refreshes in place, silently.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct AiChangedEvent;

/// Sync merged two Todo Lists with the same name: whoever is looking at `from_id` moves to
/// `into_id` (the List that was kept) and is told. `device` names the other device the List came
/// from, when it is a paired one.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct TodoListsMergedEvent {
    /// The List that went away.
    pub from_id: String,
    /// Its name.
    pub from_name: String,
    /// The List that was kept.
    pub into_id: String,
    /// Its name.
    pub into_name: String,
    /// The other device whose List it was.
    pub device: Option<String>,
}

/// A Task was deleted on another device: if it is open in the sheet, the sheet closes with a toast
/// naming `device`, and Restore brings it back (as an ordinary change).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct TodoTaskDeletedEvent {
    /// The Task.
    pub task_id: String,
    /// Its title.
    pub title: String,
    /// The device that deleted it, when it is a paired one.
    pub device: Option<String>,
}

/// A Journal Entry was deleted on another device: if it is open in the editor, the editor closes
/// with a toast naming `device`, and Restore brings it back (as an ordinary change).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct JournalEntryDeletedEvent {
    /// The Entry.
    pub entry_id: String,
    /// The Page it was on (an Entry has no title).
    pub page_date: String,
    /// The device that deleted it, when it is a paired one.
    pub device: Option<String>,
}

/// The pairing sheet's state changed (a code is shown, someone asks, it worked or failed).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct SyncPairingEvent(pub mnemora_sync::PairingView);

/// A paired device's status or sync progress changed: the Devices tab reads `sync_devices` again.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct SyncPeersEvent;

/// Sync raised, received or settled a Conflict: the Conflicts tab and Compare are stale.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct SyncConflictsEvent;

/// Activity gained lines (a session was logged, or a line arrived from another device): the
/// Activity tab reads its first page again.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct SyncActivityEvent;

/// New Conflicts arrived from another device and this device is set to say so with a toast
/// ("New conflicts: Show a toast"). Never sent while it is set to Quietly.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct SyncNewConflictsEvent {
    /// How many.
    pub count: u32,
    /// The device they came from, when known.
    pub device: Option<String>,
}

/// One step of an AI Run, with the Run and its Conversation: the page follows it live.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct AiRunEvent {
    /// The Run.
    pub run_id: String,
    /// The Conversation it answers in.
    pub conversation_id: String,
    /// What happened.
    pub event: mnemora_ai::RunEvent,
}

/// A link opened the app while it was running (the Todo widget's taps): `app_take_links` has it.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct LinkOpenedEvent;

/// Every Module in the app, in migration order. The one place Modules are registered.
fn modules() -> Vec<Box<dyn Module>> {
    vec![
        Box::new(CoreModule),
        Box::new(JournalModule),
        Box::new(TodoModule),
        Box::new(AiModule),
        Box::new(SyncModule),
    ]
}

/// Tells the screens of the Module whose data another device changed that what they show is stale.
fn announce_remote_change<R: tauri::Runtime>(handle: &tauri::AppHandle<R>, module: &str) {
    let sent = match module {
        mnemora_todo::MODULE_ID => TodoChangedEvent.emit(handle),
        mnemora_journal::MODULE_ID => JournalChangedEvent.emit(handle),
        mnemora_ai::MODULE_ID => AiChangedEvent.emit(handle),
        // The names in the Devices list can have changed with the Activity lines (a rename).
        mnemora_sync::MODULE_ID => SyncConflictsEvent
            .emit(handle)
            .and_then(|()| SyncPeersEvent.emit(handle))
            .and_then(|()| SyncActivityEvent.emit(handle)),
        _ => Ok(()),
    };
    if let Err(e) = sent {
        tracing::warn!(target: "sync", error = %e, "could not emit the change event");
    }
}

/// Turns "Sync changed a Module's data" into that Module's own change event: Todo's and the
/// Journal's screens refresh in place after every batch, silently. The Todo widget redraws once per sync session,
/// not per batch, so a big sync does not flood it.
fn follow_remote<R: tauri::Runtime>(
    bus: &EventBus,
    handle: tauri::AppHandle<R>,
    widgets: &Arc<widget::signal::WidgetSignal>,
) {
    let handle_for_merges = handle.clone();
    let handle_for_deletes = handle.clone();
    let toast_handle = handle.clone();
    let rename_handle = handle.clone();
    bus.subscribe(move |event: &RemoteChanged| announce_remote_change(&handle, &event.module));
    // Another device renamed this one: the name is saved in the device file like a rename here.
    // The file is written before the session goes on, so the name is there whenever the rename's
    // Activity line is; advertising it is off the session's thread: that holds the database,
    // which advertising the new name needs too.
    bus.subscribe(move |event: &DeviceRenamed| {
        use tauri::Manager;
        let Some(state) = rename_handle.try_state::<state::AppState>() else {
            return;
        };
        if let Err(e) = mnemora_sync::rename(&state.device, &event.name) {
            tracing::warn!(target: "sync", error = %e, "could not save the new device name");
            return;
        }
        let handle = rename_handle.clone();
        tauri::async_runtime::spawn_blocking(move || {
            use tauri::Manager;
            let Some(state) = handle.try_state::<state::AppState>() else {
                return;
            };
            // Tell the window first: advertising the new name can take a while (it waits for the
            // network), and the Devices tab must not show the old name meanwhile.
            let emit = || {
                if let Err(e) = SyncPeersEvent.emit(&handle) {
                    tracing::warn!(target: "sync", error = %e, "could not emit the device change");
                }
            };
            emit();
            state.sync.refresh_advert();
            emit();
        });
    });
    let merged_handle = handle_for_merges;
    bus.subscribe(move |event: &ListsMerged| {
        let event = TodoListsMergedEvent {
            from_id: event.from_id.clone(),
            from_name: event.from_name.clone(),
            into_id: event.into_id.clone(),
            into_name: event.into_name.clone(),
            device: event.device.clone(),
        };
        if let Err(e) = event.emit(&merged_handle) {
            tracing::warn!(target: "sync", error = %e, "could not emit the List merge event");
        }
    });
    let deleted_handle = handle_for_deletes;
    let entry_deleted_handle = deleted_handle.clone();
    bus.subscribe(move |event: &TaskDeletedRemotely| {
        let event = TodoTaskDeletedEvent {
            task_id: event.task_id.clone(),
            title: event.title.clone(),
            device: event.device.clone(),
        };
        if let Err(e) = event.emit(&deleted_handle) {
            tracing::warn!(target: "sync", error = %e, "could not emit the Task delete event");
        }
    });
    bus.subscribe(move |event: &EntryDeletedRemotely| {
        let event = JournalEntryDeletedEvent {
            entry_id: event.entry_id.clone(),
            page_date: event.page_date.clone(),
            device: event.device.clone(),
        };
        if let Err(e) = event.emit(&entry_deleted_handle) {
            tracing::warn!(target: "sync", error = %e, "could not emit the Entry delete event");
        }
    });
    bus.subscribe(move |event: &ConflictsRaised| {
        use tauri::Manager;
        // Quietly is the default: only a device set to a toast hears about new conflicts.
        let wants = toast_handle
            .try_state::<state::AppState>()
            .is_some_and(|s| s.device.get().sync.toast_conflicts);
        if !wants {
            return;
        }
        let toast = SyncNewConflictsEvent {
            count: event.count,
            device: event.device.clone(),
        };
        if let Err(e) = toast.emit(&toast_handle) {
            tracing::warn!(target: "sync", error = %e, "could not emit the new-conflicts event");
        }
    });
    let widgets = Arc::clone(widgets);
    bus.subscribe(move |event: &RemoteSessionEnded| {
        if event.modules.iter().any(|m| m == mnemora_todo::MODULE_ID) {
            widgets.todo_changed();
        }
    });
}

/// A Settings store over every Module's registered keys.
fn settings_store(
    modules: &[Box<dyn Module>],
    bus: &EventBus,
    device: &Arc<DeviceStore>,
) -> SettingsStore {
    let mut registry = SettingsRegistry::new();
    for module in modules {
        module.register_settings(&mut registry);
    }
    SettingsStore::new(Arc::new(registry), bus.clone(), Arc::clone(device))
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

/// The owner-only folder for AI keys, under the config directory (ADR 0020).
pub(crate) const AI_KEYS_DIR: &str = "ai-keys";

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
    let device = Arc::new(DeviceStore::new(paths.device_file(), device));

    let modules: Modules = Arc::new(modules());
    let module_refs: Vec<&dyn Module> = modules.iter().map(AsRef::as_ref).collect();
    let mut conn = db::open(&paths.db_file)?;
    let report = db::migrate::run(&mut conn, &module_refs, &paths.backup_dir)?;
    tracing::info!(applied = ?report.applied, backup = ?report.backup, "database ready");

    // Once per start, after migrations. A failing Module must not stop the app.
    for module in modules.iter() {
        if let Err(e) = module.on_startup(&mut conn, &device, &report) {
            tracing::warn!(module = module.id(), error = %e, "startup work failed");
        }
    }

    // No editor survives a restart: merge what a previous run was still holding back for one.
    editing::release_at_start(&mut conn, &modules, &device.device_id());

    let bus = EventBus::new();
    let settings = settings_store(&modules, &bus, &device);
    // Settings that change what the Todo widget draws redraw it too.
    let widgets = Arc::new(widget::signal::WidgetSignal::platform());
    widget::signal::follow_settings(&bus, &widgets);
    follow_remote(&bus, app.handle().clone(), &widgets);
    // Sync starts only when this device has it on; off, nothing is started at all.
    let sync = SyncRuntime::new();
    sync.reconcile(device.get().sync.enabled);
    // A change this device records wakes the session manager (a no-op while Sync is off).
    let changes = sync.local_changes();
    db::on_change_log_write(&conn, move || changes.notify_one())?;
    // Look and feel comes from the device file alone, so the first paint never waits on the database.
    let first_paint = settings.device_values();

    // Bridge core's in-process event to a typed IPC event.
    let handle = app.handle().clone();
    bus.subscribe(move |event: &SettingsChanged| {
        if let Err(e) = SettingsChangedEvent(event.clone()).emit(&handle) {
            tracing::warn!(error = %e, "could not emit settings event");
        }
    });
    builder.mount_events(app);

    let ai = ai::AiRuntime::new(paths.config_dir.join(AI_KEYS_DIR));
    let state = AppState {
        env: ENVIRONMENT,
        db: Mutex::new(conn),
        settings,
        device: Arc::clone(&device),
        paths,
        log,
        widgets,
        sync,
        power: sync_power::Power::new(),
        live_network: true,
        modules,
        bus,
        ai,
    };
    app.manage(state);
    // With Sync on, open the port and start announcing; with it off nothing is started.
    if device.get().sync.enabled {
        sync::launch_network(app.handle());
    }
    #[cfg(target_os = "android")]
    {
        widget::register_app(app.handle().clone());
        sync_power::android::register_app(app.handle().clone());
    }
    // A Todo widget placed before the app ever ran (or before an update) shows a "open Mnemora"
    // state; now that the database exists and is current, it should redraw without waiting for
    // the next Todo write.
    app.state::<AppState>().widgets.todo_changed();

    create_main_window(app, &first_paint, &device.get())?;
    Ok(())
}

fn create_main_window(
    app: &tauri::App,
    values: &std::collections::BTreeMap<String, serde_json::Value>,
    device: &DeviceSettings,
) -> Result<(), Box<dyn Error>> {
    let mut config = app
        .config()
        .app
        .windows
        .first()
        .ok_or("no window configured in tauri.conf.json")?
        .clone();
    if cfg!(feature = "sandbox") {
        config.title = "Mnemora Sandbox".into();
    }
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
    // Managed here, not in `setup`: a link can arrive before setup has finished.
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(links::PendingLinks::default())
        .invoke_handler(invoke_handler)
        .setup(move |app| setup(app, &builder))
        .build(tauri::generate_context!())
        .map(|app| {
            app.run(|handle, event| {
                #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
                if let tauri::RunEvent::Opened { urls } = &event {
                    links::accept(handle, urls.iter().map(ToString::to_string));
                }
                #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
                let _ = (handle, event);
            });
        });
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
            "appTakeLinks",
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
            "aiRunStart",
            "aiConversationGet",
            "aiRunEvent",
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
            "syncGet",
            "syncSetEnabled",
            "syncRename",
            "syncSetOption",
            "syncLeave",
            "syncDevices",
            "syncPairState",
            "syncPairShow",
            "syncPairNewCode",
            "syncPairEnter",
            "syncPairDecide",
            "syncPairCancel",
            "syncNow",
            "syncResume",
            "syncConflicts",
            "syncConflict",
            "syncConflictDraft",
            "syncConflictResolve",
            "syncPeersEvent",
            "syncConflictsEvent",
            "syncPairingEvent",
            "settingsChangedEvent",
            "linkOpenedEvent",
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
