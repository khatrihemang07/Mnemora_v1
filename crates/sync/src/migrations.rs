//! Append-only schema migrations for Sync. They run on every install, Sync on or off; the tables
//! stay empty until the first pairing.

use rusqlite_migration::{M, Migrations};

#[allow(clippy::too_many_lines)] // an append-only list
pub fn all() -> Migrations<'static> {
    Migrations::new(vec![
        // Paired devices: one row per other device in this device's group.
        M::up(
            "CREATE TABLE sync_peers(
             id TEXT PRIMARY KEY NOT NULL,
             name TEXT NOT NULL,
             paired_at INTEGER NOT NULL
         ) STRICT",
        ),
        // The engine's local bookkeeping (ADR 0017); none of it is synced. `sync_seen` is the
        // version vector: the highest sequence seen per entity and origin device, advanced in
        // the transaction that stores what it counts. `sync_field_clocks` says when each field
        // of each record last changed, as `(ms, counter, device)`; `sync_clock_marks` is how far
        // into the change log those clocks have been read, per entity.
        M::up(
            "CREATE TABLE sync_seen(
             entity TEXT NOT NULL,
             device TEXT NOT NULL,
             seq INTEGER NOT NULL,
             PRIMARY KEY(entity, device)
         ) STRICT;
         CREATE TABLE sync_field_clocks(
             entity TEXT NOT NULL,
             entity_id TEXT NOT NULL,
             field TEXT NOT NULL,
             ms INTEGER NOT NULL,
             counter INTEGER NOT NULL,
             device TEXT NOT NULL,
             PRIMARY KEY(entity, entity_id, field)
         ) STRICT;
         CREATE TABLE sync_clock_marks(
             entity TEXT PRIMARY KEY NOT NULL,
             seq INTEGER NOT NULL
         ) STRICT;",
        ),
        // Pairing (#62). A paired device is known by its public key (hex), which is what TLS
        // pins; `paired_by` is the device that paired it (this one, or the member that
        // introduced it). `sync_group` is the one group id this device belongs to, made at the
        // first pairing; no row until then.
        M::up(
            "ALTER TABLE sync_peers ADD COLUMN public_key TEXT;
         ALTER TABLE sync_peers ADD COLUMN platform TEXT NOT NULL DEFAULT 'desktop';
         ALTER TABLE sync_peers ADD COLUMN paired_by TEXT;
         CREATE TABLE sync_group(
             one INTEGER PRIMARY KEY NOT NULL CHECK(one = 1),
             id TEXT NOT NULL
         ) STRICT;",
        ),
        // Live syncing (#63), all local to this device and not synced: what the last session with a
        // paired device moved and by which route, when it was last reached, and the addresses
        // remembered for it with when each last worked.
        M::up(
            "CREATE TABLE sync_peer_status(
             device TEXT PRIMARY KEY NOT NULL,
             last_seen_at INTEGER,
             last_synced_at INTEGER,
             last_in INTEGER,
             last_out INTEGER,
             route TEXT
         ) STRICT;
         CREATE TABLE sync_peer_addresses(
             device TEXT NOT NULL,
             address TEXT NOT NULL,
             last_ok_at INTEGER,
             PRIMARY KEY(device, address)
         ) STRICT;",
        ),
        // Held changes (#65), local and not synced: what other devices sent for a field that an
        // editor holds (core::editing). The record itself is stored and counted as seen, so it
        // is never asked for again; only its values for that unit wait here until the hold ends.
        // `ms/counter/device` is the record's clock, which the unit's field clocks were raised to.
        M::up(
            "CREATE TABLE sync_held(
             entity TEXT NOT NULL,
             entity_id TEXT NOT NULL,
             unit TEXT NOT NULL,
             ms INTEGER NOT NULL,
             counter INTEGER NOT NULL,
             device TEXT NOT NULL,
             op TEXT NOT NULL,
             at INTEGER NOT NULL,
             fields TEXT NOT NULL,
             PRIMARY KEY(entity, entity_id, unit, ms, counter, device)
         ) STRICT;",
        ),
        // Conflicts (#66), synced as the change-log entity `sync_conflict`: two devices rewrote
        // the same words of a text. `a_*` is the newest version (what was applied), `b_*` the
        // other; `base` what both started from. `status` is `open` until a resolution is
        // chosen (or the record turned out to be deleted).
        M::up(
            "CREATE TABLE sync_conflicts(
             id TEXT PRIMARY KEY NOT NULL,
             target TEXT NOT NULL,
             target_id TEXT NOT NULL,
             field TEXT NOT NULL,
             base TEXT NOT NULL,
             a_text TEXT NOT NULL,
             a_device TEXT NOT NULL,
             a_at INTEGER NOT NULL,
             b_text TEXT NOT NULL,
             b_device TEXT NOT NULL,
             b_at INTEGER NOT NULL,
             raised_at INTEGER NOT NULL,
             status TEXT NOT NULL DEFAULT 'open',
             resolution TEXT,
             resolved_at INTEGER,
             resolved_by TEXT
         ) STRICT;
         CREATE INDEX sync_conflicts_target ON sync_conflicts(target, target_id);
         CREATE INDEX sync_conflicts_open ON sync_conflicts(status, raised_at);",
        ),
        // Where the replay of a text unit got to (#66 review), local and not synced: a memo of
        // `texts::fold`, so a long-edited record is not replayed from its first record on every
        // batch. `ms/counter/device/seq` is the last record replayed, `n` how many change-log
        // rows of the record sort at or before it (the memo is dropped if that count differs: a
        // record arrived late, or a session's record was rewritten), `state` the replay's result.
        M::up(
            "CREATE TABLE sync_text_replay(
             entity TEXT NOT NULL,
             entity_id TEXT NOT NULL,
             unit TEXT NOT NULL,
             ms INTEGER NOT NULL,
             counter INTEGER NOT NULL,
             device TEXT NOT NULL,
             seq INTEGER NOT NULL,
             n INTEGER NOT NULL,
             state TEXT NOT NULL,
             PRIMARY KEY(entity, entity_id, unit)
         ) STRICT;",
        ),
        // Resolved automatically (#67), synced as the change-log entity `sync_resolution` except
        // the folded "Order & preferences" lines, which are this device's own record of a sync
        // session. `kept`/`replaced` are JSON objects of the unit's values; `at` is the clock
        // time of the decision (identical on every device), `learned_at` when THIS device first
        // had the row (local, drives the "since your last visit" dot). `used_*` is set once
        // "Use ‹other› instead" was pressed. `sync_resolution_visit` is the local last-visit
        // marker.
        M::up(
            "CREATE TABLE sync_resolutions(
             id TEXT PRIMARY KEY NOT NULL,
             kind TEXT NOT NULL,
             target TEXT NOT NULL DEFAULT '',
             target_id TEXT NOT NULL DEFAULT '',
             field TEXT NOT NULL DEFAULT '',
             kept TEXT NOT NULL DEFAULT '{}',
             kept_device TEXT NOT NULL DEFAULT '',
             replaced TEXT NOT NULL DEFAULT '{}',
             replaced_device TEXT NOT NULL DEFAULT '',
             extra TEXT NOT NULL DEFAULT '',
             at INTEGER NOT NULL,
             used_at INTEGER,
             used_by TEXT,
             learned_at INTEGER NOT NULL,
             count INTEGER NOT NULL DEFAULT 0,
             detail TEXT NOT NULL DEFAULT '[]'
         ) STRICT;
         CREATE INDEX sync_resolutions_at ON sync_resolutions(at);
         CREATE TABLE sync_resolution_visit(
             one INTEGER PRIMARY KEY NOT NULL CHECK(one = 1),
             at INTEGER NOT NULL
         ) STRICT;",
        ),
        // Activity (#68), synced as the change-log entity `sync_activity` and kept forever. A row
        // is written once: `device` is the device that logged it, `peer` the other device it is
        // about (both with the name they had then, for devices this one does not know), `n_in`
        // and `n_out` what a session moved from the logger's side, `detail` its per-Module
        // breakdown (counts only), `extra` kind-specific JSON (a reason, names).
        M::up(
            "CREATE TABLE sync_activity(
             id TEXT PRIMARY KEY NOT NULL,
             kind TEXT NOT NULL,
             at INTEGER NOT NULL,
             device TEXT NOT NULL,
             device_name TEXT NOT NULL DEFAULT '',
             peer TEXT NOT NULL DEFAULT '',
             peer_name TEXT NOT NULL DEFAULT '',
             n_in INTEGER NOT NULL DEFAULT 0,
             n_out INTEGER NOT NULL DEFAULT 0,
             detail TEXT NOT NULL DEFAULT '[]',
             extra TEXT NOT NULL DEFAULT ''
         ) STRICT;
         CREATE INDEX sync_activity_at ON sync_activity(at DESC, id DESC);",
        ),
        // The device sheet, Forget and Rename (#69). `typed_address` is the Address a person typed for
        // a paired device (local). `app_version` and `data_versions` (JSON) are what the device
        // said about itself the last time it was reached (local). `sync_revocations` is synced as
        // the change-log entity `sync_revocation`: a signed decision of a member that a device is
        // out of the group (`state` revoked), or in again after pairing (`lifted`); `at` orders
        // decisions, `by_device` signed it, `signature` is hex.
        M::up(
            "ALTER TABLE sync_peers ADD COLUMN typed_address TEXT;
         ALTER TABLE sync_peer_status ADD COLUMN app_version TEXT;
         ALTER TABLE sync_peer_status ADD COLUMN data_versions TEXT;
         CREATE TABLE sync_revocations(
             device TEXT PRIMARY KEY NOT NULL,
             public_key TEXT NOT NULL,
             name TEXT NOT NULL,
             platform TEXT NOT NULL,
             state TEXT NOT NULL,
             at INTEGER NOT NULL,
             by_device TEXT NOT NULL,
             signature TEXT NOT NULL
         ) STRICT;",
        ),
        // The name a device was last renamed to, as asked for. The name shown (`name`) may carry a
        // number when two devices were given one name; deriving it from every wish in a fixed
        // order keeps the devices in agreement (#69 review). Local.
        M::up("ALTER TABLE sync_peers ADD COLUMN wanted_name TEXT;"),
        // Introductions (#70), synced as the change-log entity `sync_introduction`: a member signs
        // the key of a device it paired, so the members that were not there accept it. `at` orders
        // them, `by_device` signed it, `signature` is hex.
        M::up(
            "CREATE TABLE sync_introductions(
             device TEXT PRIMARY KEY NOT NULL,
             public_key TEXT NOT NULL,
             name TEXT NOT NULL,
             platform TEXT NOT NULL,
             at INTEGER NOT NULL,
             by_device TEXT NOT NULL,
             signature TEXT NOT NULL
         ) STRICT;",
        ),
        // Whether the signer paired the device itself; signed, and the only kind that can undo a
        // Forget (#70 review). Introductions made before this were direct pairings.
        M::up("ALTER TABLE sync_introductions ADD COLUMN direct INTEGER NOT NULL DEFAULT 1;"),
        // How far this device's own change-log records have been looked at for Value clashes with
        // records received earlier (local; one row). See `resolutions::settle_local`.
        M::up(
            "CREATE TABLE sync_local_settled(
             one INTEGER PRIMARY KEY NOT NULL CHECK (one = 1),
             seq INTEGER NOT NULL
         ) STRICT;
         INSERT INTO sync_local_settled(one, seq) SELECT 1, COALESCE(MAX(seq), 0) FROM change_log;",
        ),
    ])
}
