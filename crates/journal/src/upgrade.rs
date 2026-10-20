//! Steps that run when the Journal arrives on a database that already existed.

use mnemora_core::db::migrate::MigrationReport;
use mnemora_core::error::AppResult;
use mnemora_core::settings::save_if_unset;
use rusqlite::Connection;
use serde_json::json;

use crate::MODULE_ID;

/// Core registers Home + Journal as the phone-tabs default for new installs. An existing install
/// that never saved its tabs would silently gain a Journal tab, so when the Journal first
/// arrives on a database Core had already migrated, its unsaved tabs are pinned to Home only.
/// A saved choice is never touched.
pub fn keep_existing_phone_tabs(conn: &Connection, report: &MigrationReport) -> AppResult<()> {
    let first_run_of = |id: &str| {
        report
            .applied
            .iter()
            .any(|(m, from, _)| m == id && *from == 0)
    };
    if first_run_of(MODULE_ID) && !first_run_of("core") {
        save_if_unset(conn, "core", "phone_tabs", &json!(["home"]))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mnemora_core::db::{self, migrate};
    use mnemora_core::events::EventBus;
    use mnemora_core::module::{CoreModule, Module};
    use mnemora_core::settings::{SettingsChange, SettingsRegistry, SettingsStore};
    use serde_json::Value;

    use super::*;
    use crate::JournalModule;

    fn tabs(conn: &Connection) -> Value {
        let mut reg = SettingsRegistry::new();
        CoreModule.register_settings(&mut reg);
        JournalModule.register_settings(&mut reg);
        let store = SettingsStore::new(Arc::new(reg), EventBus::new());
        store.get_all(conn).unwrap().values["core.phone_tabs"].clone()
    }

    /// Migrates like app start does: the given Modules, then the startup hook.
    fn start(conn: &mut Connection, dir: &std::path::Path, modules: &[&dyn Module]) {
        let report = migrate::run(conn, modules, dir).unwrap();
        keep_existing_phone_tabs(conn, &report).unwrap();
    }

    #[test]
    fn fresh_install_gets_home_and_journal() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        start(&mut conn, dir.path(), &[&CoreModule, &JournalModule]);
        assert_eq!(tabs(&conn), json!(["home", "journal"]));
    }

    #[test]
    fn existing_install_without_saved_tabs_stays_home_only() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        start(&mut conn, dir.path(), &[&CoreModule]);
        start(&mut conn, dir.path(), &[&CoreModule, &JournalModule]);
        assert_eq!(tabs(&conn), json!(["home"]));
        // Later starts change nothing.
        start(&mut conn, dir.path(), &[&CoreModule, &JournalModule]);
        assert_eq!(tabs(&conn), json!(["home"]));
    }

    #[test]
    fn existing_install_with_saved_tabs_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let mut conn = db::open_in_memory().unwrap();
        start(&mut conn, dir.path(), &[&CoreModule]);
        let mut reg = SettingsRegistry::new();
        CoreModule.register_settings(&mut reg);
        SettingsStore::new(Arc::new(reg), EventBus::new())
            .save_batch(
                &mut conn,
                &[SettingsChange {
                    key: "core.phone_tabs".into(),
                    value: json!(["home", "settings"]),
                }],
            )
            .unwrap();
        start(&mut conn, dir.path(), &[&CoreModule, &JournalModule]);
        assert_eq!(tabs(&conn), json!(["home", "settings"]));
    }
}
