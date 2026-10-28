//! Runs real AI answers over a copy of the Sandbox's seeded data and writes markdown reports:
//! `cargo run -p mnemora-app --features seed --example ai_qa -- --out <folder>`. It needs the `seed`
//! feature, which keeps the harness out of the app. The flags and the safety rules are in
//! `mnemora_lib::ai_qa`.

#[cfg(feature = "seed")]
fn main() {
    std::process::exit(mnemora_lib::ai_qa::run_cli(std::env::args().skip(1)));
}

#[cfg(not(feature = "seed"))]
fn main() {
    eprintln!(
        "ai_qa needs the seed feature: cargo run -p mnemora-app --features seed --example ai_qa"
    );
    std::process::exit(2);
}
