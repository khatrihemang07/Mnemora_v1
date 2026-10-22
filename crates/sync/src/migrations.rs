//! Append-only schema migrations for Sync. They run on every install, Sync on or off; the tables
//! stay empty until the first pairing.

use rusqlite_migration::{M, Migrations};

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
    ])
}
