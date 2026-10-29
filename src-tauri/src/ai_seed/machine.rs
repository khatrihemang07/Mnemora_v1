//! This machine's facts the seed needs: the platform data folder and the time zone.

use std::fs;
use std::path::PathBuf;

/// This user's platform data folder, where the app's folders live: `~/Library/Application Support`
/// on macOS, `%APPDATA%` on Windows, `$XDG_DATA_HOME` or `~/.local/share` elsewhere.
pub fn platform_data_root() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        Ok(PathBuf::from(home).join("Library/Application Support"))
    }
    #[cfg(target_os = "windows")]
    {
        let roaming = std::env::var_os("APPDATA").ok_or("APPDATA is not set")?;
        Ok(PathBuf::from(roaming))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(data) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            return Ok(PathBuf::from(data));
        }
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        Ok(PathBuf::from(home).join(".local/share"))
    }
}

/// This machine's IANA zone: `TZ` when it names one, else what `/etc/localtime` points at.
pub fn system_zone() -> Option<String> {
    if let Ok(tz) = std::env::var("TZ") {
        let tz = tz.trim_start_matches(':');
        if !tz.is_empty() {
            return Some(tz.to_owned());
        }
    }
    let target = fs::read_link("/etc/localtime").ok()?;
    let text = target.to_string_lossy().into_owned();
    text.split_once("zoneinfo/")
        .map(|(_, zone)| zone.to_owned())
}
