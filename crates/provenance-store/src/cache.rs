pub(crate) mod connection;
mod gaps;
mod health;
mod impact;
mod materialize;
mod prime;
mod projection_digest;
mod projection_families;
pub mod read;
mod traceability;

pub(crate) use connection::permission_failure;
pub use connection::{
    open_cache, open_cache_with, open_existing_cache, open_immutable_cache, open_stored_cache,
    CacheConnection, WalSwitchRetry,
};
pub use gaps::*;
pub use health::*;
pub use impact::*;
pub(crate) use materialize::catch_up_with_guard;
pub use materialize::{
    catch_up_state, materialize_empty_state, materialize_state, scope_ids, unit_digest, units_for,
    CatchUpReport, Unit, UnitHashError,
};
pub use prime::*;
pub use projection_digest::{
    family_content_digests, revision_digest, revision_digest_from_stored_rows, FamilyContentDigest,
};
pub use projection_families::ProjectionFamily;
pub use traceability::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MaterializeReport {
    pub records_loaded: u64,
    pub migrations_applied: Vec<String>,
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
pub(crate) mod tests;
