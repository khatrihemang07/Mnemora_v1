//! Search matching shared by Modules: query parsing into safe FTS5 expressions, typo tolerance,
//! and marking matches in text. A Module owns its own index and SQL; the matching rules live
//! here so every Module's search behaves the same.

pub mod fold;
pub mod fuzzy;
pub mod mark;
pub mod plain;
pub mod query;

pub use fuzzy::{near_terms, tolerance};
pub use mark::{Matcher, Span, highlight, window};
pub use query::{Term, exact_match, expand_match, parse};
