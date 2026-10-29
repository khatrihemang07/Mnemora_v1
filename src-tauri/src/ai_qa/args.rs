//! The flags of `ai_qa`, read into `Args`. A flag takes a value unless it is `--help` or `--dump-last`.

use std::path::PathBuf;

use super::knobs::{Off, Variant};
use crate::qa_cli::{unknown, value};

/// The usage line `ai_qa` prints for `--help` and for bad arguments.
pub const USAGE: &str =
    "usage: cargo run -p mnemora-app --features seed --example ai_qa -- --out <folder> [options]
  --out <folder>      where the reports are written (required)
  --variant <v>       before, after or both (default: both)
  --provider <name>   the Provider to ask (default: the Sandbox's only one, or the one used last)
  --model <id>        the Model id (default: the Model last used with that Provider)
  --zone <zone>       the calendar's IANA zone (default: this Mac's)
  --questions <file>  a JSON list of questions (default: the built-in ten)
  --data-dir <folder> the Sandbox data folder (default: the one the Sandbox app would use)
  --base <folder>     the platform data folder the Sandbox lives in (default: this user's)
  --off <knobs>       switch off for every variant: final, intent, changes, paging (default: none)
  --fixture-skip-catchup   the Sandbox copy must be seeded without the catch-up day (seed-sandbox --skip-catchup)
  --dump-first-on-zero-calls  add the first request of each run that made no Tool call on its first Step to its report";

/// Command-line options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    /// `-h` or `--help` was given: print the usage and stop.
    pub help: bool,
    /// The platform data folder the Sandbox lives in (`--base`). This user's when None.
    pub base: Option<PathBuf>,
    /// The Sandbox data folder (`--data-dir`). The one the Sandbox app would use when None.
    pub data_dir: Option<PathBuf>,
    /// The calendar's IANA zone (`--zone`). This Mac's when None.
    pub zone: Option<String>,
    /// The Provider's name or id (`--provider`).
    pub provider: Option<String>,
    /// The Model id (`--model`). The one last used with the Provider when None.
    pub model: Option<String>,
    /// The variants to run (`--variant`). Both, before then after, when none is named.
    pub variants: Vec<Variant>,
    /// A JSON list of questions (`--questions`). The built-in ten when None.
    pub questions: Option<PathBuf>,
    /// The folder the reports are written to (`--out`, required).
    pub out: Option<PathBuf>,
    /// The knobs switched off for every variant (`--off`).
    pub off: Off,
    /// Whether to write the last request of the first question to a dump file (`--dump-last`).
    pub dump: bool,
    /// Whether each run that made no Tool call on its first Step adds that first request to its
    /// report (`--dump-first-on-zero-calls`).
    pub dump_first_on_zero_calls: bool,
}

/// Reads the flags. `--variant` is `before`, `after` or `both` (the default).
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Args, String> {
    let mut options = Args {
        help: false,
        base: None,
        data_dir: None,
        zone: None,
        provider: None,
        model: None,
        variants: vec![Variant::Before, Variant::After],
        questions: None,
        out: None,
        off: Off::default(),
        dump: false,
        dump_first_on_zero_calls: false,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => options.help = true,
            "--base" => options.base = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--data-dir" => options.data_dir = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--zone" => options.zone = Some(value(&mut args, &arg)?),
            "--provider" => options.provider = Some(value(&mut args, &arg)?),
            "--model" => options.model = Some(value(&mut args, &arg)?),
            "--questions" => options.questions = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--out" => options.out = Some(PathBuf::from(value(&mut args, &arg)?)),
            "--off" => options.off = Off::parse(&value(&mut args, &arg)?)?,
            "--dump-last" => options.dump = true,
            "--dump-first-on-zero-calls" => options.dump_first_on_zero_calls = true,
            "--fixture-skip-catchup" => options.off.catchup = true,
            "--variant" => {
                options.variants = match value(&mut args, &arg)?.as_str() {
                    "before" => vec![Variant::Before],
                    "after" => vec![Variant::After],
                    "both" => vec![Variant::Before, Variant::After],
                    other => {
                        return Err(format!(
                            "--variant takes before, after or both, not {other}"
                        ));
                    }
                };
            }
            other => return Err(unknown(other)),
        }
    }
    Ok(options)
}
