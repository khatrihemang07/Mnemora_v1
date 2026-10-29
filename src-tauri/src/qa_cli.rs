//! The command-line shape the two headless tools share: `ai_qa` (`examples/ai_qa.rs`) and the
//! Sandbox seed (`examples/seed_sandbox.rs`). Both read `--flag value` pairs, refuse an unknown
//! flag, print their usage on bad arguments and exit 0 (done or help), 1 (refused or failed) or 2
//! (bad arguments).

/// The value that follows `flag`, or a refusal that names the flag when none was given.
pub fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
}

/// The refusal for a flag the tool does not know.
pub fn unknown(arg: &str) -> String {
    format!("unknown argument {arg}")
}

/// Runs a tool from the command line and returns its process exit code. `parsed` is the parsed
/// flags, or the refusal of bad ones (printed with `tool` and `usage`, exit 2). `wants_help` ends the
/// run after the usage is printed (exit 0). Otherwise `run` runs: its text goes to stdout (exit 0),
/// or its refusal goes to stderr, prefixed with `tool` (exit 1).
pub fn exit_code<O>(
    tool: &str,
    usage: &str,
    parsed: Result<O, String>,
    wants_help: impl Fn(&O) -> bool,
    run: impl FnOnce(&O) -> Result<String, String>,
) -> i32 {
    let options = match parsed {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{tool}: {message}\n{usage}");
            return 2;
        }
    };
    if wants_help(&options) {
        println!("{usage}");
        return 0;
    }
    match run(&options) {
        Ok(text) => {
            println!("{text}");
            0
        }
        Err(message) => {
            eprintln!("{tool}: {message}");
            1
        }
    }
}
