//! The seed's command line: reads the flags, runs the seed on the Sandbox and says what it did.

use std::path::PathBuf;

use mnemora_core::local_time;
use mnemora_core::paths::AppPaths;
use mnemora_core::util::now_ms;

use super::args::{Options, USAGE, parse_args};
use super::database::DbState;
use super::guard::sandbox_paths;
use super::machine::{platform_data_root, system_zone};
use super::seed::{SeedOutcome, SeedReport, seed_at_with};
use crate::qa_cli::exit_code;

/// Runs the seed from the command line and returns the process exit code: 0 when it wrote or found
/// the fixture already there, 1 when it refused or failed, 2 for bad arguments.
pub fn run_cli<I: IntoIterator<Item = String>>(args: I) -> i32 {
    exit_code(
        "seed-sandbox",
        USAGE,
        parse_args(args),
        |options| options.help,
        run,
    )
}

fn run(options: &Options) -> Result<String, String> {
    let real_base = platform_data_root()?;
    let (paths, production) = sandbox_paths(
        &real_base,
        options.base.as_deref(),
        options.data_dir.as_deref(),
    )?;
    let zone = options
        .zone
        .clone()
        .or_else(system_zone)
        .ok_or("this Mac's time zone is not an IANA name; pass --zone")?;
    let now = now_ms();
    let report = seed_at_with(
        &paths,
        &production,
        now,
        &zone,
        options.reset,
        options.skip_catchup,
    )?;
    Ok(describe(&paths, &production, now, &zone, &report))
}

/// The lines `run_cli` prints: where the seed wrote, what it found and what it did.
fn describe(
    paths: &AppPaths,
    production: &[PathBuf],
    now: i64,
    zone: &str,
    report: &SeedReport,
) -> String {
    let when = local_time::minute(now, zone).unwrap_or_else(|| "unknown".to_owned());
    let checked: Vec<String> = production.iter().map(|p| p.display().to_string()).collect();
    let mut lines = vec![
        format!("Sandbox database: {}", paths.db_file.display()),
        format!(
            "  production folders checked, none touched: {}",
            checked.join(", ")
        ),
        format!("  zone {zone}, written as of {when}"),
        format!("  database before: {}", describe_state(report.before)),
    ];
    match &report.outcome {
        SeedOutcome::Written {
            events,
            items,
            skipped,
            left_out,
            counts,
        } => {
            lines.push(format!(
                "  wrote {events} events as {items} items; {skipped} events after now were left out"
            ));
            if *left_out > 0 {
                lines.push(format!(
                    "  the catch-up day was left out: {left_out} of its events were not written"
                ));
            }
            let rows: Vec<String> = counts.iter().map(|(t, n)| format!("{t} {n}")).collect();
            lines.push(format!("  rows now: {}", rows.join(", ")));
        }
        SeedOutcome::AlreadySeeded => lines.push(
            "  nothing written: the fixture is already there (pass --reset to seed it again)"
                .to_owned(),
        ),
    }
    lines.join("\n")
}

fn describe_state(state: DbState) -> &'static str {
    match state {
        DbState::Missing => "none (created)",
        DbState::Empty => "empty",
        DbState::Seeded => "the fixture",
        DbState::Other => "rows that are not the fixture (replaced: --reset)",
    }
}
