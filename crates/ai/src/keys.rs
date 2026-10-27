//! API keys: one owner-only file per Provider, in the app's config directory (ADR 0020). Written
//! atomically, never synced, never logged, and never read back to the screen: the screen learns
//! only whether one is set.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use mnemora_core::error::{AppError, AppResult, ErrorCode};

/// Where the keys are kept: `<dir>/<provider id>`.
pub struct KeyStore {
    dir: PathBuf,
}

impl KeyStore {
    /// Keys in `dir`, which is created on the first save.
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// The key for a Provider, if one is saved.
    pub fn get(&self, provider_id: &str) -> AppResult<Option<String>> {
        match fs::read_to_string(self.path(provider_id)?) {
            Ok(text) => {
                let key = text.trim();
                Ok((!key.is_empty()).then(|| key.to_owned()))
            }
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io_error(e)),
        }
    }

    /// Whether a key is saved for the Provider.
    pub fn has(&self, provider_id: &str) -> AppResult<bool> {
        Ok(self.get(provider_id)?.is_some())
    }

    /// Saves a key, replacing any earlier one. The file is owner-only from the moment it exists,
    /// and is renamed into place so a crash never leaves half a key.
    pub fn set(&self, provider_id: &str, key: &str) -> AppResult<()> {
        let path = self.path(provider_id)?;
        fs::create_dir_all(&self.dir).map_err(io_error)?;
        let tmp = self.dir.join(format!(".{provider_id}.tmp"));
        // A stale temp file from a crash keeps its old mode, so it goes first.
        let _ = fs::remove_file(&tmp);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp).map_err(io_error)?;
        file.write_all(key.as_bytes()).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        drop(file);
        fs::rename(&tmp, &path).map_err(io_error)
    }

    /// Deletes a Provider's key. Nothing to delete is not an error.
    pub fn remove(&self, provider_id: &str) -> AppResult<()> {
        match fs::remove_file(self.path(provider_id)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_error(e)),
        }
    }

    /// `<dir>/<id>`, refusing anything but the characters a Provider id is made of.
    fn path(&self, provider_id: &str) -> AppResult<PathBuf> {
        let safe = !provider_id.is_empty()
            && provider_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-');
        if !safe {
            return Err(AppError::new(ErrorCode::Internal, "Unknown Provider"));
        }
        Ok(Path::new(&self.dir).join(provider_id))
    }
}

fn io_error(e: std::io::Error) -> AppError {
    // The error text names only the path, never the key.
    AppError::new(ErrorCode::Io, "Could not save the key on this device.").with_details(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_round_trips_and_can_be_replaced_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let keys = KeyStore::new(dir.path().join("ai-keys"));
        assert_eq!(keys.get("p1").unwrap(), None);
        keys.set("p1", "first-key").unwrap();
        assert_eq!(keys.get("p1").unwrap().as_deref(), Some("first-key"));
        keys.set("p1", "second-key").unwrap();
        assert_eq!(keys.get("p1").unwrap().as_deref(), Some("second-key"));
        assert!(keys.has("p1").unwrap());
        keys.remove("p1").unwrap();
        keys.remove("p1").unwrap();
        assert!(!keys.has("p1").unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn the_key_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let keys = KeyStore::new(dir.path().join("ai-keys"));
        keys.set("p1", "secret").unwrap();
        let mode = fs::metadata(dir.path().join("ai-keys/p1"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_provider_id_that_could_escape_the_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let keys = KeyStore::new(dir.path().to_path_buf());
        assert!(keys.set("../evil", "k").is_err());
        assert!(keys.get("a/b").is_err());
        assert!(keys.get("").is_err());
    }
}
