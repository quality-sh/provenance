//! This crate is an implementation part of Provenance. Provenance is pre-release, and this API can
//! change without compatibility shims. Use `provenance-sdk` for Rust.
//!
//! This crate is the repository engine. It owns the use cases and calls storage through a
//! port. The engine runs where its storage is: in the process over local disk now, and as
//! Wasm in a Durable Object over the SQLite of that object later.
//!
//! The areas of the crate:
//!
//! - `repository` owns the identity and the lifecycle of a repository.
//! - `operations` (with `operations::catalog`) and `review` hold the use cases.
//! - `state_store` holds validation and the record writers.
//! - `storage` is the port and its adapters.
//! - `publication`, `jsonl`, `shards` and `layout` are the sections, stages
//!   and shard paths of the engine.
//! - `families` holds the record family table.
//! - `cache::connection` manages the SQLite connections.
//! - `cache::materialize` and `cache::read` are the projection.
//!
//! Without the `host` feature the crate has the write engine: a repository
//! over any storage adapter, the record writers, stages, review saves and
//! the merge logic, and it builds for wasm32-unknown-unknown. The `host`
//! feature adds the local-disk adapter, git, the dictionary index files,
//! the projection, the queries and the use cases that read through them.

#[cfg(feature = "host")]
pub mod cache;
pub mod canonical_digest;
pub mod code_refs;
#[cfg(feature = "host")]
pub mod current_schema;
pub mod dictionary_reference;
pub mod families;
#[cfg(all(feature = "host", feature = "test-fixture"))]
pub mod fixture_probe;
#[cfg(feature = "host")]
pub mod git_process;
pub mod jsonl;
pub mod layout;
#[cfg(feature = "host")]
mod long_path;
pub mod merge;
#[cfg(feature = "host")]
pub mod operations;
pub mod publication;
pub mod repository;
#[cfg(feature = "host")]
pub mod repository_init;
pub mod review;
#[cfg(feature = "host")]
pub mod review_changes;
pub mod settings;
pub mod shards;
#[cfg(feature = "host")]
pub mod stale;
pub mod state_store;
pub mod statement_analysis;
pub mod storage;
mod test_probes;
#[cfg(feature = "host")]
pub mod user_cache;
#[cfg(all(test, feature = "host"))]
extern crate self as provenance_store;
// Each module of unit tests that needs `host` has a plain `#[cfg(test)]` and
// a second attribute for the host gate. Clippy treats only a module with a
// plain `cfg(test)` as test code.
#[cfg(all(test, feature = "host"))]
mod test_support;

/// The validator version that a completed projection rebuild records.
///
/// 1. Graph and ideation validation when catch-up reads changed units in place.
///
/// Increase this version when a validator change requires existing scopes to be checked again.
#[cfg(feature = "host")]
pub const VALIDATION_VERSION: u32 = 2;

pub mod write_error;
