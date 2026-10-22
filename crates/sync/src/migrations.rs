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
    ])
}
