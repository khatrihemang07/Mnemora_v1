//! Mnemora core: everything shared by every Module and the app shell.
//!
//! Rust owns all data and rules; the UI is a thin typed view over this crate.

#![forbid(unsafe_code)]

pub mod changelog;
pub mod db;
pub mod device;
pub mod editing;
pub mod env;
pub mod error;
pub mod events;
pub mod hlc;
pub mod logging;
pub mod module;
pub mod paths;
pub mod search;
pub mod settings;
pub mod syncable;
pub mod text;
pub mod util;
