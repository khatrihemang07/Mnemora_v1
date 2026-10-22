//! The compile-time contract every Module implements.

use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};
use serde_json::json;

use crate::db::migrate::MigrationReport;
use crate::device::DeviceStore;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::settings::{SettingsRegistry, validate};
use crate::syncable::{Applied, RemoteChange, Settled, SyncEntity};

/// A self-contained area of the app. Registered in one place by the app crate.
/// Modules never import each other; they share only core contracts and the event bus.
pub trait Module: Send + Sync {
    /// Stable id, also the Settings key prefix and the migration namespace.
    fn id(&self) -> &'static str;

    /// Whether the Module is on until the user switches it off (true, the default) or off until
    /// they opt in (Sync). A default-off Module is enabled when listed in the
    /// `core.enabled_optional_modules` Preference; Sync's own switch is a Device setting instead.
    fn default_enabled(&self) -> bool {
        true
    }

    /// This Module's own, independently versioned schema migrations.
    fn migrations(&self) -> Migrations<'static>;

    /// Declares this Module's Settings keys, defaults and validators.
    fn register_settings(&self, reg: &mut SettingsRegistry);

    /// The entities this Module keeps in step between devices, with the class of every field.
    /// None by default: a Module that declares none never syncs. See [`crate::syncable`].
    fn sync_entities(&self) -> &'static [SyncEntity] {
        &[]
    }

    /// Applies field values received from another device to one of this Module's entities,
    /// through its own repository and validation (ADR 0001): creating the entity when it is
    /// new here. Only the values that won the merge are passed. The result is not a new local
    /// change: nothing is recorded in the change log (Sync records the received change itself)
    /// and nothing new is stamped, but the Module's usual caches and indexes are kept in step.
    /// Returns the entity's full state afterwards. Never log the values.
    fn apply_remote(&self, _conn: &Connection, _change: &RemoteChange<'_>) -> AppResult<Applied> {
        Err(AppError::new(
            ErrorCode::Internal,
            "This Module does not take part in Sync",
        ))
    }

    /// Runs right after a remote change was applied and stored, in the same transaction, for
    /// work that must follow the stored record: a rule that decides something by comparing the
    /// change with the rest of the data and records the outcome as ordinary local changes
    /// (Todo's List name clash). Anything recorded here is stamped later than the record it
    /// follows, so it wins over it on every device. Idempotent: running it again after nothing
    /// changed records nothing.
    fn settle_remote(&self, _conn: &Connection, _change: &RemoteChange<'_>) -> AppResult<Settled> {
        Ok(Settled::default())
    }

    /// Work to do once per app start, after every Module's migrations ran (housekeeping such as
    /// purging old rows, or upgrade steps keyed on what `report` says this run applied). No
    /// polling: nothing calls this again. Never log user content.
    fn on_startup(
        &self,
        _conn: &mut Connection,
        _device: &DeviceStore,
        _report: &MigrationReport,
    ) -> AppResult<()> {
        Ok(())
    }
}

/// Core's own Module: the `settings` table and the app-wide preferences.
pub struct CoreModule;

impl Module for CoreModule {
    fn id(&self) -> &'static str {
        "core"
    }

    fn migrations(&self) -> Migrations<'static> {
        Migrations::new(vec![
            M::up(
                "CREATE TABLE settings(
                 module TEXT NOT NULL,
                 key TEXT NOT NULL,
                 value TEXT NOT NULL,
                 updated_at INTEGER NOT NULL,
                 PRIMARY KEY(module, key)
             ) STRICT",
            ),
            // App-wide change log (ADR 0008). The partial unique index is what lets one editing
            // session keep a single record per entity.
            M::up(
                "CREATE TABLE change_log(
                 id TEXT PRIMARY KEY NOT NULL,
                 module TEXT NOT NULL,
                 entity TEXT NOT NULL,
                 entity_id TEXT NOT NULL,
                 op TEXT NOT NULL,
                 session_id TEXT,
                 snapshot TEXT NOT NULL,
                 at INTEGER NOT NULL
             ) STRICT;
             CREATE INDEX change_log_entity ON change_log(entity, entity_id);
             CREATE UNIQUE INDEX change_log_session
                 ON change_log(entity, entity_id, session_id) WHERE session_id IS NOT NULL;",
            ),
            // Sync (ADR 0017): every record knows its origin device, hybrid logical clock
            // (ms + counter) and sequence. `seq` is this database's own append order and also
            // bumps when a session's record is rewritten; `origin_seq` is the sequence at the
            // origin device (equal to `seq` for records made here, kept as received for records
            // from other devices). Existing rows get this device (filled at startup, once the
            // device id is known), the clock `at`/0, and a sequence in (at, insertion) order,
            // which is the order Task History has always shown.
            M::up(
                "ALTER TABLE change_log ADD COLUMN origin_device TEXT NOT NULL DEFAULT '';
             ALTER TABLE change_log ADD COLUMN hlc_ms INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE change_log ADD COLUMN hlc_counter INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE change_log ADD COLUMN seq INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE change_log ADD COLUMN origin_seq INTEGER NOT NULL DEFAULT 0;
             UPDATE change_log SET hlc_ms = at;
             UPDATE change_log SET seq = ordered.n, origin_seq = ordered.n
                 FROM (SELECT id, ROW_NUMBER() OVER (ORDER BY at, rowid) AS n FROM change_log) AS ordered
                 WHERE ordered.id = change_log.id;
             CREATE UNIQUE INDEX change_log_seq ON change_log(seq);
             CREATE UNIQUE INDEX change_log_origin ON change_log(origin_device, origin_seq);
             CREATE INDEX change_log_clock
                 ON change_log(entity, entity_id, hlc_ms, hlc_counter, origin_device, origin_seq);
             CREATE TABLE device_clock(
                 id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
                 device_id TEXT NOT NULL,
                 ms INTEGER NOT NULL,
                 counter INTEGER NOT NULL,
                 seq INTEGER NOT NULL
             ) STRICT;
             INSERT INTO device_clock(id, device_id, ms, counter, seq)
                 SELECT 1, '', COALESCE(MAX(hlc_ms), 0), 0, COALESCE(MAX(seq), 0) FROM change_log;",
            ),
            // Sync (ADR 0017): `fields` holds only what a record changed, as `{field: value}`
            // with the origin's values (a received record keeps them as sent, so any device can
            // relay it). Older rows have none, which reads as "every field of the snapshot".
            // The indexes serve the per-entity clock fold and the per-origin stream and vector.
            M::up(
                "ALTER TABLE change_log ADD COLUMN fields TEXT;
             CREATE INDEX change_log_entity_seq ON change_log(entity, seq);
             CREATE INDEX change_log_entity_origin ON change_log(entity, origin_device, origin_seq);",
            ),
            // Holds that keep Sync from replacing text being typed (local, never synced; see
            // `editing`). Cleared at every start.
            M::up(
                "CREATE TABLE editing_sessions(
                 entity TEXT NOT NULL,
                 entity_id TEXT NOT NULL,
                 scope TEXT NOT NULL,
                 touched_at INTEGER NOT NULL,
                 PRIMARY KEY(entity, entity_id, scope)
             ) STRICT;",
            ),
        ])
    }

    fn on_startup(
        &self,
        conn: &mut Connection,
        device: &DeviceStore,
        _report: &MigrationReport,
    ) -> AppResult<()> {
        // First: every later write is stamped with this device.
        crate::hlc::set_device_id(conn, &device.device_id())?;
        // No editor survives a restart: nothing is held back any more.
        crate::editing::clear(conn)?;
        crate::settings::move_look_and_feel_to_device(conn, device)
    }

    fn sync_entities(&self) -> &'static [SyncEntity] {
        crate::settings::sync::ENTITIES
    }

    fn apply_remote(&self, conn: &Connection, change: &RemoteChange<'_>) -> AppResult<Applied> {
        match change.entity {
            crate::settings::ENTITY => crate::settings::sync::apply(conn, change),
            _ => Err(AppError::new(
                ErrorCode::Internal,
                "Core does not sync that kind of record",
            )),
        }
    }

    fn register_settings(&self, reg: &mut SettingsRegistry) {
        let m = self.id();
        reg.register_device(
            m,
            "theme",
            json!("system"),
            validate::one_of(&["system", "light", "dark", "oled"]),
        );
        reg.register_device(
            m,
            "density",
            json!("comfortable"),
            validate::one_of(&["comfortable", "compact"]),
        );
        reg.register_device(
            m,
            "text_size",
            json!("m"),
            validate::one_of(&["s", "m", "l"]),
        );
        reg.register_device(m, "reduce_motion", json!(false), validate::boolean());
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
        // New installs get Home + Journal + Todo; a saved choice always wins over this default.
        // An install that never saved its tabs gets Todo appended by this default alone; one that
        // did (the Journal pins ["home"] for those that never chose) keeps what it has.
        reg.register_device(
            m,
            "phone_tabs",
            json!(["home", "journal", "todo"]),
            validate::string_list(4),
        );
        // Set when the tabs are saved from Settings: a saved value without it was only ever
        // pinned by an upgrade, so a Module arriving later may still add its tab.
        reg.register_device(m, "phone_tabs_edited", json!(false), validate::boolean());
        // Ids of Modules switched off. A list of the exceptions, so a new Module is on by default.
        reg.register(m, "disabled_modules", json!([]), validate::string_list(64));
        // Ids of default-off Modules the user opted into. Enabled = (default on and not in
        // `disabled_modules`) or (default off and listed here).
        reg.register(
            m,
            "enabled_optional_modules",
            json!([]),
            validate::string_list(64),
        );
    }
}
