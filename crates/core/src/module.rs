//! The compile-time contract every Module implements.

use rusqlite_migration::{M, Migrations};
use serde_json::json;

use crate::settings::{SettingsRegistry, validate};

/// A self-contained area of the app. Registered in one place by the app crate.
/// Modules never import each other; they share only core contracts and the event bus.
pub trait Module: Send + Sync {
    /// Stable id, also the Settings key prefix and the migration namespace.
    fn id(&self) -> &'static str;

    /// This Module's own, independently versioned schema migrations.
    fn migrations(&self) -> Migrations<'static>;

    /// Declares this Module's Settings keys, defaults and validators.
    fn register_settings(&self, reg: &mut SettingsRegistry);
}

/// Core's own Module: the `settings` table and the app-wide preferences.
pub struct CoreModule;

impl Module for CoreModule {
    fn id(&self) -> &'static str {
        "core"
    }

    fn migrations(&self) -> Migrations<'static> {
        Migrations::new(vec![M::up(
            "CREATE TABLE settings(
                 module TEXT NOT NULL,
                 key TEXT NOT NULL,
                 value TEXT NOT NULL,
                 updated_at INTEGER NOT NULL,
                 PRIMARY KEY(module, key)
             ) STRICT",
        )])
    }

    fn register_settings(&self, reg: &mut SettingsRegistry) {
        let m = self.id();
        reg.register(
            m,
            "theme",
            json!("system"),
            validate::one_of(&["system", "light", "dark", "oled"]),
        );
        reg.register(
            m,
            "density",
            json!("comfortable"),
            validate::one_of(&["comfortable", "compact"]),
        );
        reg.register(
            m,
            "text_size",
            json!("m"),
            validate::one_of(&["s", "m", "l"]),
        );
        reg.register(m, "reduce_motion", json!(false), validate::boolean());
        // Empty means "use the OS first name".
        reg.register(m, "display_name", json!(""), validate::text(60));
        reg.register(
            m,
            "time_format",
            json!("24h"),
            validate::one_of(&["24h", "12h"]),
        );
        reg.register(
            m,
            "date_format",
            json!("long"),
            validate::one_of(&["long", "short", "iso"]),
        );
        reg.register(
            m,
            "week_start",
            json!("mon"),
            validate::one_of(&["mon", "sun"]),
        );
        reg.register(m, "phone_tabs", json!(["home"]), validate::string_list(4));
        // Ids of Modules switched off. A list of the exceptions, so a new Module is on by default.
        reg.register(m, "disabled_modules", json!([]), validate::string_list(64));
    }
}
