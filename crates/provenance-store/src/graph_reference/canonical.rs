//! Canonical bytes for the graph-reference module.
//!
//! The one home of canonical serialization is [`crate::canonical_digest`];
//! this file keeps the module's historical `pub(super)` surface, mapping
//! serialization failures the way `GraphReferenceError` always reported
//! them, so `export.rs` keeps its exact behavior.

use serde::Serialize;

use super::GraphReferenceError;

pub(super) fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, GraphReferenceError> {
    crate::canonical_digest::canonical_bytes(value).map_err(super::incomplete)
}

pub(super) use crate::canonical_digest::{digest, sha256};
