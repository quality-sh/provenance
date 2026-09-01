//! Graph-reference views over the crate's canonical digest machinery.
//!
//! The serialization and hashing live in `crate::canonical_digest`, the one
//! home for canonical bytes. This module keeps the graph-reference error
//! shape: a value that cannot serialize is an incomplete document here, not a
//! bare serde error.

use super::{incomplete, GraphReferenceError};
use serde::Serialize;

pub(super) fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, GraphReferenceError> {
    crate::canonical_digest::canonical_bytes(value).map_err(incomplete)
}

pub(super) fn digest(bytes: &[u8]) -> String {
    crate::canonical_digest::digest(bytes)
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    crate::canonical_digest::sha256(bytes)
}
