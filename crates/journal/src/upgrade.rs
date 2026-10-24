//! Steps that run when the Journal arrives on a database that already existed.

use mnemora_core::db::migrate::MigrationReport;
use mnemora_core::device::DeviceStore;
use mnemora_core::error::AppResult;

use crate::MODULE_ID;

/// Core registers Home + Journal as the phone-tabs default for new installs. An existing install
/// that never saved its tabs would silently gain a Journal tab, so when the Journal first
/// arrives on a database Core had already migrated, its unsaved tabs (a Device setting) are
/// pinned to Home only. A saved choice is never touched.
pub fn keep_existing_phone_tabs(device: &DeviceStore, report: &MigrationReport) -> AppResult<()> {
    let first_run_of = |id: &str| {
        report
            .applied
            .iter()
            .any(|(m, from, _)| m == id && *from == 0)
    };
    if first_run_of(MODULE_ID) && !first_run_of("core") && device.get().look.phone_tabs.is_none() {
        device.update(|d| d.look.phone_tabs = Some(vec!["home".to_owned()]))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use mnemora_core::db::{self, migrate};
    use mnemora_core::device::DeviceSettings;
    use mnemora_core::module::{CoreModule, Module};
    use rusqlite::Connection;

    use super::*;
    use crate::JournalModule;

    fn tabs(device: &DeviceStore) -> Option<Vec<String>> {
        device.get().look.phone_tabs
    }

    /// Migrates like app start does: the given Modules, then the startup hook.
    fn start(
        conn: &mut Connection,
        device: &DeviceStore,
        dir: &std::path::Path,
        modules: &[&dyn Module],
    ) {
        let report = migrate::run(conn, modules, dir).unwrap();
        keep_existing_phone_tabs(device, &report).unwrap();
    }

    fn device(dir: &std::path::Path) -> DeviceStore {
        DeviceStore::new(dir.join("device.toml"), DeviceSettings::default())
    }

    #[test]
    fn fresh_install_keeps_the_core_default() {
        let dir = tempfile::tempdir().unwrap();
        let (mut conn, device) = (db::open_in_memory().unwrap(), device(dir.path()));
        start(
            &mut conn,
            &device,
            dir.path(),
            &[&CoreModule, &JournalModule],
        );
        assert_eq!(tabs(&device), None, "nothing is saved, the default applies");
    }

    #[test]
    fn existing_install_without_saved_tabs_stays_home_only() {
        let dir = tempfile::tempdir().unwrap();
        let (mut conn, device) = (db::open_in_memory().unwrap(), device(dir.path()));
        start(&mut conn, &device, dir.path(), &[&CoreModule]);
        start(
            &mut conn,
            &device,
            dir.path(),
            &[&CoreModule, &JournalModule],
        );
        assert_eq!(tabs(&device), Some(vec!["home".to_owned()]));
        // Later starts change nothing.
        start(
            &mut conn,
            &device,
            dir.path(),
            &[&CoreModule, &JournalModule],
        );
        assert_eq!(tabs(&device), Some(vec!["home".to_owned()]));
    }

    #[test]
    fn existing_install_with_saved_tabs_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let (mut conn, device) = (db::open_in_memory().unwrap(), device(dir.path()));
        start(&mut conn, &device, dir.path(), &[&CoreModule]);
        let saved = vec!["home".to_owned(), "settings".to_owned()];
        device
            .update(|d| d.look.phone_tabs = Some(saved.clone()))
            .unwrap();
        start(
            &mut conn,
            &device,
            dir.path(),
            &[&CoreModule, &JournalModule],
        );
        assert_eq!(tabs(&device), Some(saved));
    }
}
