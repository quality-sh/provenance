//! The evidence diff gate over a git commit range.
//!
//! The gate classifies evidence paths against a named base and head; it
//! performs no review and no re-extraction, and it writes nothing.

pub mod gate;
pub mod git;
