//! Seeding a Sandbox database: the checks and the decision before anything is written (`seed_at`),
//! then the fixture's write through the Modules' services (`write_fixture`).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;

use mnemora_core::db;
use mnemora_core::device::{DeviceSettings, DeviceStore};
use mnemora_core::local_time;
use mnemora_core::module::Module;
use mnemora_core::paths::AppPaths;

use super::database::{DbState, inspect, remove_database};
use super::driver::SeedDriver;
use super::err;
use super::guard::refuse_production;
use super::marker::record_counts;
use super::plan::{Plan, plan};
use crate::ai_fixture::{self, Driver};

/// What a seed did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeedOutcome {
    /// The fixture was written.
    Written {
        /// Fixture events written.
        events: usize,
        /// Keyed items written (Lists, Tasks and Entries).
        items: usize,
        /// Fixture events left out because they fall after the seed's `now`.
        skipped: usize,
        /// Rows per data table after the seed.
        counts: BTreeMap<String, i64>,
    },
    /// Nothing was written: the database already holds the fixture.
    AlreadySeeded,
}

/// The state the database was in, and what the seed did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedReport {
    /// The state the database was in before the seed looked at it.
    pub before: DbState,
    /// What the seed did about it.
    pub outcome: SeedOutcome,
}

/// Seeds the database of `paths` with the fixture, as of `now` in `zone`. `production` are the
/// folders of Production's data (see `production_roots`); a target inside any of them is refused
/// before anything else is looked at.
pub fn seed_at(
    paths: &AppPaths,
    production: &[PathBuf],
    now: i64,
    zone: &str,
    reset: bool,
) -> Result<SeedReport, String> {
    refuse_production(
        &[
            paths.data_dir.as_path(),
            paths.config_dir.as_path(),
            paths.db_file.as_path(),
        ],
        production,
    )?;
    if local_time::day(now, zone).is_none() {
        return Err(format!("{zone} is not an IANA time zone; pass --zone"));
    }
    let before = inspect(&paths.db_file)?;
    let outcome = match plan(before, reset)? {
        Plan::Keep => SeedOutcome::AlreadySeeded,
        step => {
            if step == Plan::Reset {
                remove_database(&paths.db_file)?;
            }
            write_fixture(paths, now, zone)?
        }
    };
    Ok(SeedReport { before, outcome })
}

/// Opens (creating) the database, migrates it with every Module as the app does at start, then
/// replays the fixture through the Modules' services and records the row counts.
fn write_fixture(paths: &AppPaths, now: i64, zone: &str) -> Result<SeedOutcome, String> {
    let mut conn = db::open(&paths.db_file).map_err(err)?;
    let modules = crate::modules();
    let refs: Vec<&dyn Module> = modules.iter().map(AsRef::as_ref).collect();
    let report = db::migrate::run(&mut conn, &refs, &paths.backup_dir).map_err(err)?;
    let device = DeviceStore::new(
        paths.device_file(),
        DeviceSettings::load(&paths.device_file()),
    );
    for module in &modules {
        module
            .on_startup(&mut conn, &device, &report)
            .map_err(err)?;
    }

    let driver = SeedDriver::new(conn, Arc::new(AtomicI64::new(now)))?;
    let loaded = ai_fixture::load(&driver, now, zone)?;
    let mut recorded = Err("the database was not reached".to_owned());
    driver.with_db(&mut |conn| recorded = record_counts(conn));
    let counts = recorded?;
    Ok(SeedOutcome::Written {
        events: loaded.events(),
        items: loaded.len(),
        skipped: loaded.skipped(),
        counts,
    })
}
