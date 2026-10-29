//! Fills the Sandbox database with the recent-changes fixture, dated from today: `make seed-sandbox`.
//! It needs the `seed` feature, which keeps the seed out of the app. The safety rules and flags are
//! in `mnemora_lib::ai_seed`.

#[cfg(feature = "seed")]
fn main() {
    std::process::exit(mnemora_lib::ai_seed::run_cli(std::env::args().skip(1)));
}

#[cfg(not(feature = "seed"))]
fn main() {
    eprintln!(
        "seed_sandbox needs the seed feature: cargo run -p mnemora-app --features seed --example seed_sandbox"
    );
    std::process::exit(2);
}
