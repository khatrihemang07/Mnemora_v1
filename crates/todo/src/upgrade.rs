//! Steps that run when Todo first arrives on a database that already existed.

use mnemora_core::db::migrate::MigrationReport;
use mnemora_core::device::DeviceStore;
use mnemora_core::error::AppResult;
use mnemora_core::settings::stored;
use rusqlite::Connection;
use serde_json::json;

use crate::MODULE_ID;

/// The most tabs the phone tab bar holds.
const MAX_TABS: usize = 4;

/// An install that never edited its phone tabs gets Todo appended, once. "Never edited" is a
/// saved value (a Device setting) without the `core.phone_tabs_edited` marker (which saving the tabs from Settings
/// sets) that is exactly what an upgrade pins: nothing saved, so the new default already has Todo,
/// or `["home"]` (the Journal's upgrade) or `["home", "journal"]`, which gain Todo at the end. Any
/// other saved list counts as edited, marker or not, and stays as it is. It never runs again, so
/// removing the tab later sticks; it adds nothing to four tabs or past a switched-off Todo.
pub fn append_todo_tab(
    conn: &Connection,
    device: &DeviceStore,
    report: &MigrationReport,
) -> AppResult<()> {
    let first_run_of = |id: &str| {
        report
            .applied
            .iter()
            .any(|(m, from, _)| m == id && *from == 0)
    };
    if !first_run_of(MODULE_ID) || first_run_of("core") {
        return Ok(());
    }
    let look = device.get().look;
    if look.phone_tabs_edited == Some(true) {
        return Ok(());
    }
    let off = stored(conn, "core", "disabled_modules")?
        .and_then(|v| v.as_array().map(|a| a.contains(&json!(MODULE_ID))))
        .unwrap_or(false);
    let Some(mut tabs) = look.phone_tabs else {
        return Ok(());
    };
    let home = || "home".to_owned();
    let pinned = tabs == [home()] || tabs == [home(), "journal".to_owned()];
    if off || !pinned || tabs.len() >= MAX_TABS {
        return Ok(());
    }
    tabs.push(MODULE_ID.to_owned());
    device.update(|d| d.look.phone_tabs = Some(tabs))
}
