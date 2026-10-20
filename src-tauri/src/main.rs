//! Desktop entry point; the real app lives in the library so mobile can share it.

// Prevents an extra console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    mnemora_lib::run();
}
