//! The Production refusal: the folders of Production's data, the check that a target lies outside
//! them, and the Sandbox's paths, which are checked before anything is read or written.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use mnemora_core::device::DeviceSettings;
use mnemora_core::env::Environment;
use mnemora_core::paths::AppPaths;

/// The folders that hold Production's data, read from the real platform data folder: Production's
/// own folder, and the data folder its device file moved it to, if it did. Reads only.
pub fn production_roots(real_base: &Path) -> Vec<PathBuf> {
    let production = AppPaths::resolve(Environment::Production, real_base, None);
    let mut roots = vec![real_base.join(Environment::Production.app_identifier())];
    if let Some(moved) = DeviceSettings::load(&production.device_file()).data_dir_override() {
        roots.push(moved);
    }
    roots
}

/// Refuses when any of `targets` lies inside one of `production`'s folders, after symlinks are
/// resolved, so a path that leads into Production by a link is refused too.
pub fn refuse_production(targets: &[&Path], production: &[PathBuf]) -> Result<(), String> {
    let roots: Vec<PathBuf> = production.iter().map(|root| resolved(root)).collect();
    for target in targets {
        let real = resolved(target);
        for (root, real_root) in production.iter().zip(&roots) {
            if real.starts_with(real_root) {
                return Err(format!(
                    "refused: {} is inside Production's data ({}). The seed never writes Production data.",
                    target.display(),
                    root.display()
                ));
            }
        }
    }
    Ok(())
}

/// `path` with symlinks resolved as far as it exists: the longest existing ancestor is made real and
/// the rest appended, so a folder that does not exist yet can still be compared with Production's.
fn resolved(path: &Path) -> PathBuf {
    let mut rest: Vec<&OsStr> = Vec::new();
    let mut at = path;
    loop {
        if let Ok(real) = fs::canonicalize(at) {
            return rest.iter().rev().fold(real, |acc, part| acc.join(part));
        }
        match (at.parent(), at.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name);
                at = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// The Sandbox's paths, resolved as the app would resolve them: under `base` (default: `real_base`,
/// the platform data folder) and in `data_dir` (default: the folder the Sandbox's device file
/// moved it to). Refused, before anything is read or written there, when any of them lies inside
/// Production's folders, which are read from `real_base`. Returns the paths and those folders.
pub fn sandbox_paths(
    real_base: &Path,
    base: Option<&Path>,
    data_dir: Option<&Path>,
) -> Result<(AppPaths, Vec<PathBuf>), String> {
    let base = base.map_or_else(|| real_base.to_path_buf(), Path::to_path_buf);
    let boot = AppPaths::resolve(Environment::Sandbox, &base, None);
    let data_dir = data_dir
        .map(Path::to_path_buf)
        .or_else(|| DeviceSettings::load(&boot.device_file()).data_dir_override());
    let paths = AppPaths::resolve(Environment::Sandbox, &base, data_dir.as_deref());
    let production = production_roots(real_base);
    refuse_production(
        &[
            paths.data_dir.as_path(),
            paths.config_dir.as_path(),
            paths.db_file.as_path(),
        ],
        &production,
    )?;
    Ok((paths, production))
}
