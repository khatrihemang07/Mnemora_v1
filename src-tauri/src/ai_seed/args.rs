//! The flags of the Sandbox seed, read into `Options`.

use std::path::PathBuf;

use crate::qa_cli::{unknown, value};

/// The usage line the seed prints for `--help` and for bad arguments.
pub(super) const USAGE: &str = "usage: make seed-sandbox ARGS=\"[--zone <IANA zone>] [--data-dir <folder>] [--base <folder>] [--reset] [--skip-catchup]\"
  --zone          the zone the fixture is written in (default: this Mac's)
  --data-dir      the Sandbox data folder (default: the one the Sandbox app would use)
  --base          the platform data folder the Sandbox lives in (default: this user's)
  --reset         delete the Sandbox database and seed again, when it holds anything else
  --skip-catchup  leave the catch-up day (6 October for a seed on 25 October) out of the fixture";

/// Command-line options.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Options {
    /// `-h` or `--help` was given: print the usage and stop.
    pub help: bool,
    /// The platform data folder the Sandbox lives in (`--base`). This user's when None.
    pub base: Option<PathBuf>,
    /// The Sandbox data folder (`--data-dir`). The one the Sandbox app would use when None.
    pub data_dir: Option<PathBuf>,
    /// The zone the fixture is written in (`--zone`). This Mac's when None.
    pub zone: Option<String>,
    /// Delete the Sandbox database and seed again when it holds rows that are not the fixture's
    /// (`--reset`).
    pub reset: bool,
    /// Leave the catch-up day out of the fixture (`--skip-catchup`). The database records it.
    pub skip_catchup: bool,
}

/// Reads `--reset`, `--zone <zone>`, `--data-dir <folder>`, `--base <folder>` and `--help`.
pub(super) fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => options.help = true,
            "--reset" => options.reset = true,
            "--skip-catchup" => options.skip_catchup = true,
            "--base" => options.base = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--data-dir" => options.data_dir = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--zone" => options.zone = Some(value(&mut args, &arg)?),
            other => return Err(unknown(other)),
        }
    }
    Ok(options)
}
