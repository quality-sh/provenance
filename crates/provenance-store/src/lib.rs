//! This crate is an implementation part of Provenance. Provenance is pre-release, and this API can
//! change without compatibility shims. Use `provenance-sdk` for Rust.

pub mod cache;
pub mod canonical_digest;
pub mod code_refs;
pub mod current_schema;
pub mod dictionary_reference;
#[cfg(feature = "test-fixture")]
pub mod fixture_probe;
pub mod graph_reference;
pub mod jsonl;
pub mod layout;
pub mod merge;
pub mod operations;
pub mod publication;
pub mod repository_init;
pub mod review;
pub mod settings;
pub mod shards;
pub mod stale;
pub mod state_store;
pub mod statement_analysis;
mod test_probes;
#[cfg(test)]
extern crate self as provenance_store;
#[cfg(test)]
mod test_support;

/// The validator version that a completed projection rebuild records.
///
/// 1. Graph and ideation validation when catch-up reads changed units in place.
///
/// Increase this version when a validator change requires existing scopes to be checked again.
pub const VALIDATION_VERSION: u32 = 2;

pub mod write_error;
