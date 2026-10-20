//! Refuses to start when the build's Environment and the running app disagree, so a Sandbox
//! build can never write into Production data (or the reverse).

use mnemora_core::env::Environment;

/// Checks that this build is running as the app its Environment expects.
///
/// Desktop and iOS compare the Tauri config identifier. Android can't: the Tauri CLI requires the
/// identifier to match the Kotlin package, so Sandbox keeps the Production identifier and is
/// separated by its Android package (`applicationIdSuffix`), which the OS reflects in `data_dir`.
pub fn check(
    env: Environment,
    identifier: &str,
    data_dir: &str,
    android: bool,
) -> Result<(), String> {
    let expected = env.app_identifier();
    if android {
        // Trailing slash so the Production package doesn't match inside the Sandbox path.
        if data_dir.contains(&format!("/{expected}/"))
            || data_dir.ends_with(&format!("/{expected}"))
        {
            return Ok(());
        }
        return Err(format!(
            "environment mismatch: {} build running from {data_dir}",
            env.as_str()
        ));
    }
    if identifier == expected {
        Ok(())
    } else {
        Err(format!(
            "environment mismatch: config identifier {identifier} vs build {expected}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROD_DIR: &str = "/data/user/0/com.hemang.mnemora/files";
    const SANDBOX_DIR: &str = "/data/user/0/com.hemang.mnemora.sandbox/files";

    #[test]
    fn desktop_matches_on_identifier() {
        assert!(check(Environment::Production, "com.hemang.mnemora", "", false).is_ok());
        assert!(
            check(
                Environment::Sandbox,
                "com.hemang.mnemora.sandbox",
                "",
                false
            )
            .is_ok()
        );
    }

    #[test]
    fn desktop_rejects_sandbox_config_without_sandbox_build() {
        assert!(
            check(
                Environment::Production,
                "com.hemang.mnemora.sandbox",
                "",
                false
            )
            .is_err()
        );
    }

    #[test]
    fn android_matches_on_package_data_dir() {
        assert!(
            check(
                Environment::Production,
                "com.hemang.mnemora",
                PROD_DIR,
                true
            )
            .is_ok()
        );
        assert!(
            check(
                Environment::Sandbox,
                "com.hemang.mnemora",
                SANDBOX_DIR,
                true
            )
            .is_ok()
        );
    }

    #[test]
    fn android_rejects_cross_environment_installs() {
        assert!(check(Environment::Sandbox, "com.hemang.mnemora", PROD_DIR, true).is_err());
        assert!(
            check(
                Environment::Production,
                "com.hemang.mnemora",
                SANDBOX_DIR,
                true
            )
            .is_err()
        );
    }
}
