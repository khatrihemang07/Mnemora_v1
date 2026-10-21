//! Steps that run when Todo first arrives on a database that already existed.

use mnemora_core::db::migrate::MigrationReport;
use mnemora_core::error::AppResult;
use mnemora_core::settings::stored;
use mnemora_core::util::now_ms;
use rusqlite::Connection;
use serde_json::{Value, json};

use crate::MODULE_ID;

/// The most tabs the phone tab bar holds.
const MAX_TABS: usize = 4;

/// An install that never edited its phone tabs gets Todo appended, once. "Never edited" is a
/// saved value without the `core.phone_tabs_edited` marker (which saving the tabs from Settings
/// sets): either nothing is saved, so the new default already has Todo, or the Journal's upgrade
/// pinned it (`["home"]`), which becomes Home, Todo. It never runs again, so removing the tab
/// later sticks; it adds nothing to four tabs or past a switched-off Todo.
pub fn append_todo_tab(conn: &Connection, report: &MigrationReport) -> AppResult<()> {
    let first_run_of = |id: &str| {
        report
            .applied
            .iter()
            .any(|(m, from, _)| m == id && *from == 0)
    };
    if !first_run_of(MODULE_ID) || first_run_of("core") {
        return Ok(());
    }
    if stored(conn, "core", "phone_tabs_edited")? == Some(json!(true)) {
        return Ok(());
    }
    let off = stored(conn, "core", "disabled_modules")?
        .and_then(|v| v.as_array().map(|a| a.contains(&json!(MODULE_ID))))
        .unwrap_or(false);
    let Some(Value::Array(mut tabs)) = stored(conn, "core", "phone_tabs")? else {
        return Ok(());
    };
    if off || tabs.len() >= MAX_TABS || tabs.contains(&json!(MODULE_ID)) {
        return Ok(());
    }
    tabs.push(json!(MODULE_ID));
    conn.execute(
        "UPDATE settings SET value = ?1, updated_at = ?2 WHERE module = 'core' AND key = 'phone_tabs'",
        rusqlite::params![Value::Array(tabs).to_string(), now_ms()],
    )?;
    Ok(())
}
