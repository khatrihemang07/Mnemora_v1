//! Where the app keeps its files, per Environment.

use std::path::{Path, PathBuf};

use crate::env::Environment;

/// Resolved directories and files for one Environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    /// Holds the database; may be overridden by the user via device settings.
    pub data_dir: PathBuf,
    /// Holds `device.toml`. Never relocatable, since it says where everything else is.
    pub config_dir: PathBuf,
    /// Rolling log files.
    pub log_dir: PathBuf,
    /// Pre-migration and manual database backups.
    pub backup_dir: PathBuf,
    /// The `SQLite` file.
    pub db_file: PathBuf,
}

impl AppPaths {
    /// Lays out paths under `base/<app identifier>/`, so Production and Sandbox never share a file.
    /// `data_override` relocates only the data (and therefore backups) directory.
    pub fn resolve(env: Environment, base: &Path, data_override: Option<&Path>) -> Self {
        let root = base.join(env.app_identifier());
        let data_dir = data_override.map_or_else(|| root.join("data"), Path::to_path_buf);
        Self {
            config_dir: root.join("config"),
            log_dir: root.join("logs"),
            backup_dir: data_dir.join("backups"),
            db_file: data_dir.join("mnemora.db"),
            data_dir,
        }
    }

    /// Path of the device settings file.
    pub fn device_file(&self) -> PathBuf {
        self.config_dir.join("device.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_and_sandbox_are_disjoint() {
        let base = Path::new("/base");
        let p = AppPaths::resolve(Environment::Production, base, None);
        let s = AppPaths::resolve(Environment::Sandbox, base, None);
        assert_eq!(p.data_dir, Path::new("/base/com.hemang.mnemora/data"));
        assert_eq!(
            s.data_dir,
            Path::new("/base/com.hemang.mnemora.sandbox/data")
        );
        assert_ne!(p.db_file, s.db_file);
        assert_ne!(p.log_dir, s.log_dir);
        assert_ne!(p.config_dir, s.config_dir);
    }

    #[test]
    fn layout_under_identifier_root() {
        let p = AppPaths::resolve(Environment::Production, Path::new("/b"), None);
        assert_eq!(p.config_dir, Path::new("/b/com.hemang.mnemora/config"));
        assert_eq!(p.log_dir, Path::new("/b/com.hemang.mnemora/logs"));
        assert_eq!(
            p.backup_dir,
            Path::new("/b/com.hemang.mnemora/data/backups")
        );
        assert_eq!(
            p.db_file,
            Path::new("/b/com.hemang.mnemora/data/mnemora.db")
        );
        assert_eq!(
            p.device_file(),
            Path::new("/b/com.hemang.mnemora/config/device.toml")
        );
    }

    #[test]
    fn data_override_moves_db_and_backups_only() {
        let p = AppPaths::resolve(
            Environment::Production,
            Path::new("/b"),
            Some(Path::new("/elsewhere")),
        );
        assert_eq!(p.data_dir, Path::new("/elsewhere"));
        assert_eq!(p.db_file, Path::new("/elsewhere/mnemora.db"));
        assert_eq!(p.backup_dir, Path::new("/elsewhere/backups"));
        assert_eq!(p.config_dir, Path::new("/b/com.hemang.mnemora/config"));
        assert_eq!(p.log_dir, Path::new("/b/com.hemang.mnemora/logs"));
    }
}
