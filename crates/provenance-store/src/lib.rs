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
//! - `publication`, `jsonl`, `shards` and `layout` are the local-disk mechanics.
//! - `cache::connection` manages the SQLite connections.
//! - `cache::materialize` and `cache::read` are the projection.
//!
//! The `host` feature turns on the engine and the local-disk mechanics.
//! Without it the crate has `storage` (the port and the memory adapter),
//! `canonical_digest` and `code_refs`, and it builds for
//! wasm32-unknown-unknown.

#[cfg(feature = "host")]
pub mod cache;
pub mod canonical_digest;
pub mod code_refs;
#[cfg(feature = "host")]
pub mod current_schema;
#[cfg(feature = "host")]
pub mod dictionary_reference;
#[cfg(all(feature = "host", feature = "test-fixture"))]
pub mod fixture_probe;
#[cfg(feature = "host")]
pub mod git_process;
#[cfg(feature = "host")]
pub mod jsonl;
#[cfg(feature = "host")]
pub mod layout;
#[cfg(feature = "host")]
mod long_path;
#[cfg(feature = "host")]
pub mod merge;
#[cfg(feature = "host")]
pub mod operations;
#[cfg(feature = "host")]
pub mod publication;
#[cfg(feature = "host")]
pub mod repository;
#[cfg(feature = "host")]
pub mod repository_init;
#[cfg(feature = "host")]
pub mod review;
#[cfg(feature = "host")]
pub mod review_changes;
#[cfg(feature = "host")]
pub mod settings;
#[cfg(feature = "host")]
pub mod shards;
#[cfg(feature = "host")]
pub mod stale;
#[cfg(feature = "host")]
pub mod state_store;
#[cfg(feature = "host")]
pub mod statement_analysis;
pub mod storage;
#[cfg(feature = "host")]
mod test_probes;
#[cfg(feature = "host")]
pub mod user_cache;
#[cfg(all(test, feature = "host"))]
extern crate self as provenance_store;
#[cfg(all(test, feature = "host"))]
mod test_support;

/// The validator version that a completed projection rebuild records.
///
/// 1. Graph and ideation validation when catch-up reads changed units in place.
///
/// Increase this version when a validator change requires existing scopes to be checked again.
#[cfg(feature = "host")]
pub const VALIDATION_VERSION: u32 = 2;

#[cfg(feature = "host")]
pub mod write_error;
