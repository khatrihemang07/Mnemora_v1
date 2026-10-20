//! The Environment the app runs against, fixed at build time.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Which data set the app uses. Chosen by the `sandbox` cargo feature of the app crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    /// The user's real data.
    Production,
    /// Separate, disposable data.
    Sandbox,
}

impl Environment {
    /// Reverse-DNS app identifier; also the folder name under each platform data root.
    pub const fn app_identifier(self) -> &'static str {
        match self {
            Self::Production => "com.hemang.mnemora",
            Self::Sandbox => "com.hemang.mnemora.sandbox",
        }
    }

    /// Human-facing app name.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Production => "Mnemora",
            Self::Sandbox => "Mnemora Sandbox",
        }
    }

    /// Short label for the UI badge; `None` when no badge should show.
    pub const fn badge(self) -> Option<&'static str> {
        match self {
            Self::Production => None,
            Self::Sandbox => Some("SANDBOX"),
        }
    }

    /// Stable lowercase name, used for `data-env` on the document root.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Sandbox => "sandbox",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_identity() {
        let e = Environment::Production;
        assert_eq!(e.app_identifier(), "com.hemang.mnemora");
        assert_eq!(e.display_name(), "Mnemora");
        assert_eq!(e.badge(), None);
        assert_eq!(e.as_str(), "production");
    }

    #[test]
    fn sandbox_identity() {
        let e = Environment::Sandbox;
        assert_eq!(e.app_identifier(), "com.hemang.mnemora.sandbox");
        assert_eq!(e.display_name(), "Mnemora Sandbox");
        assert_eq!(e.badge(), Some("SANDBOX"));
        assert_eq!(e.as_str(), "sandbox");
    }

    #[test]
    fn serializes_snake_case() {
        let json = serde_json::to_string(&Environment::Sandbox).unwrap();
        assert_eq!(json, "\"sandbox\"");
    }
}
