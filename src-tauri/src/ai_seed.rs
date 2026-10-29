//! The Sandbox seed: fills a Sandbox database with the recent-changes fixture, dated from today, so
//! that yesterday has data to read. Run through `examples/seed_sandbox.rs` (`make seed-sandbox`);
//! the app never calls it.
//!
//! Safety, in order, before anything is written: the target folders must not be inside Production's
//! data (its folder under the platform data root, or the folder its device file moved it to); the
//! database must not be in use by another connection; and it must be missing, empty, or hold exactly
//! what an earlier seed wrote. Anything else is refused unless `--reset` is given, which deletes the
//! Sandbox database files (never a Production file) and seeds again.
//!
//! The parts, in `ai_seed/`: `args` (the flags), `machine` (this Mac's data folder and zone), `guard`
//! (the Production refusal and the Sandbox's paths), `database` (what state the database is in),
//! `marker` (the record that says it holds the fixture), `plan` (what to do about that state),
//! `seed` (the checks and the write), `driver` (the fixture's commands through the services) and
//! `cli` (the command line).

mod args;
mod cli;
mod database;
mod driver;
mod guard;
mod machine;
mod marker;
mod plan;
mod seed;

pub use cli::run_cli;
pub use database::{DbState, inspect, remove_database};
pub use guard::{production_roots, refuse_production, sandbox_paths};
pub use machine::{platform_data_root, system_zone};
pub use seed::{SeedOptions, SeedOutcome, SeedReport, leaves_out_catchup, seed_at};

/// A failure as the text the command line prints.
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
