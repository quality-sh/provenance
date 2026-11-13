#[cfg(feature = "host")]
pub(crate) mod completion;
#[cfg(feature = "host")]
pub(crate) mod connection;
mod gaps;
#[cfg(feature = "host")]
mod health;
mod impact;
// Without `host`, the catch-up and the rebuild build but have no entry
// point: each entry takes the publication guard, whose wait for the
// section lock runs on the blocking pool.
#[cfg_attr(not(feature = "host"), allow(dead_code))]
mod materialize;
mod prime;
mod projection_digest;
pub mod read;

pub use crate::families::ProjectionFamily;
#[cfg(feature = "host")]
pub(crate) use completion::CompletionTimedOut;
#[cfg(feature = "host")]
pub(crate) use connection::permission_failure;
#[cfg(feature = "host")]
pub use connection::{
    open_cache, open_cache_with, open_existing_cache, open_immutable_cache, open_stored_cache,
    CacheConnection, WalSwitchRetry, COMPLETION_DEADLINE,
};
pub use gaps::*;
#[cfg(feature = "host")]
pub use health::*;
pub use impact::*;
#[cfg(test)]
#[cfg(feature = "host")]
pub(crate) use materialize::catch_up_with_guard;
#[cfg(feature = "host")]
pub(crate) use materialize::catch_up_work;
pub(crate) use materialize::stored_validation_version;
pub use materialize::{
    catch_up_in_publish, scope_ids, unit_digest, units_for, CatchUpReport, Unit, UnitHashError,
};
#[cfg(feature = "host")]
pub use materialize::{catch_up_state, materialize_empty_state, materialize_state};
pub(crate) use materialize::{unit_stored_digest, view_stored_digests};
pub use prime::*;
pub use projection_digest::{
    family_content_digests, revision_digest, revision_digest_from_stored_rows, FamilyContentDigest,
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MaterializeReport {
    pub records_loaded: u64,
    pub cache_recreated: bool,
}

pub(crate) fn serde_name<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_value(value)?.as_str().unwrap().to_string())
}

/// A quoted SQL identifier: `key`, `field`, `before`, and `after` are
/// column names and SQL keywords both.
pub(crate) fn quoted(identifier: &str) -> String {
    format!("\"{identifier}\"")
}

#[cfg(test)]
#[cfg(feature = "host")]
pub(crate) mod tests;
