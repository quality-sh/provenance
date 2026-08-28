mod gaps;
mod health;
mod impact;
mod materialize;
mod prime;
mod projection_digest;
pub(crate) mod projection_families;
mod traceability;

pub use gaps::*;
pub use health::*;
pub use impact::*;
pub use materialize::{
    catch_up_state, catch_up_state_under_guard, materialize_empty_state, materialize_state,
    CatchUpReport,
};
pub use prime::*;
pub use projection_digest::projection_digest;
pub use traceability::*;

use crate::layout::ProvenanceLayout;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MaterializeReport {
    pub records_loaded: u64,
    pub migrations_applied: Vec<String>,
    /// The revision serial this rebuild committed, zero when no rows
    /// loaded and no revision was stamped.
    pub serial: i64,
    /// The projection digest the rebuild stamped, empty when unstamped.
    pub digest: String,
    /// The projection instance the serial belongs to.
    pub instance_id: String,
}

pub async fn open_cache(layout: &ProvenanceLayout) -> anyhow::Result<SqlitePool> {
    std::fs::create_dir_all(layout.cache_dir())?;
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", layout.cache_db_path()))?
        .create_if_missing(true);
    Ok(SqlitePool::connect_with(options).await?)
}

pub(crate) fn serde_name<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_value(value)?.as_str().unwrap().to_string())
}

#[cfg(test)]
pub(crate) mod tests;
