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
    ])
}
