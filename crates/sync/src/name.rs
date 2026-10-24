//! This device's name: what the other devices call it.

use crate::error::SyncError;

/// Longest device name, in characters.
pub const MAX_NAME_CHARS: usize = 40;

/// Used when the platform reports no usable name.
const FALLBACK_NAME: &str = "This device";

/// Checks a name typed by the user and returns it trimmed.
pub fn validate_name(raw: &str) -> Result<String, SyncError> {
    let name = raw.trim();
    match name.chars().count() {
        0 => Err(SyncError::Validation("Give this device a name")),
        n if n > MAX_NAME_CHARS => Err(SyncError::Validation(
            "A device name is at most 40 characters",
        )),
        _ => Ok(name.to_owned()),
    }
}

/// The name to prefill: the computer name on a computer, the model on an Android phone.
pub fn default_device_name() -> String {
    tidy(platform_name().as_deref().unwrap_or_default())
}

/// Squeezes whitespace and cuts to the length limit, so a long computer name still fits.
fn tidy(raw: &str) -> String {
    let squeezed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = squeezed.chars().take(MAX_NAME_CHARS).collect();
    let cut = cut.trim_end().to_owned();
    if cut.is_empty() {
        FALLBACK_NAME.to_owned()
    } else {
        cut
    }
}

#[cfg(not(target_os = "android"))]
fn platform_name() -> Option<String> {
    whoami::devicename().ok()
}

/// The phone's model, which Android keeps in a system property that `getprop` (in /system/bin,
/// runnable by any app) prints; if that fails, whatever `whoami` reports.
#[cfg(target_os = "android")]
fn platform_name() -> Option<String> {
    std::process::Command::new("getprop")
        .arg("ro.product.model")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| whoami::devicename().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_trimmed_and_must_have_one_to_forty_characters() {
        assert_eq!(validate_name("  Pixel 8 ").unwrap(), "Pixel 8");
        assert!(validate_name("").is_err());
        assert!(validate_name("   ").is_err());
        assert!(
            validate_name(&"é".repeat(40)).is_ok(),
            "40 characters, not bytes"
        );
        assert!(validate_name(&"a".repeat(41)).is_err());
    }

    #[test]
    fn a_platform_name_is_squeezed_cut_and_never_empty() {
        assert_eq!(tidy("  Hemang's \n MacBook   Pro "), "Hemang's MacBook Pro");
        assert_eq!(tidy(&"x".repeat(60)).chars().count(), MAX_NAME_CHARS);
        assert_eq!(tidy(""), FALLBACK_NAME);
        assert_eq!(tidy(" \t"), FALLBACK_NAME);
        assert!(validate_name(&tidy(&format!("{} {}", "a".repeat(39), "b"))).is_ok());
    }

    #[test]
    fn the_default_name_is_always_a_valid_name() {
        assert!(validate_name(&default_device_name()).is_ok());
    }
}
