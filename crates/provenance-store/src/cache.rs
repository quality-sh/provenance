mod gaps;
mod health;
mod impact;
mod materialize;
mod prime;
mod projection_digest;
mod projection_families;
mod traceability;

pub use gaps::*;
pub use health::*;
pub use impact::*;
pub(crate) use materialize::catch_up_with_guard;
pub use materialize::{
    catch_up_state, materialize_empty_state, materialize_state, unit_digest, units_for,
    CatchUpReport, Unit,
};
pub use prime::*;
pub use projection_digest::{
    family_content_digests, revision_digest, revision_digest_from_stored_rows, FamilyContentDigest,
};
pub use projection_families::ProjectionFamily;
pub use traceability::*;

use crate::layout::ProvenanceLayout;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MaterializeReport {
    pub records_loaded: u64,
    pub migrations_applied: Vec<String>,
}

/// How long `open_cache` keeps trying while another process holds a
/// DELETE-mode file open across the switch to WAL.
const WAL_SWITCH_ATTEMPTS: u32 = 50;
const WAL_SWITCH_PAUSE: Duration = Duration::from_millis(100);

/// Opens the cache database, creating it when absent.
///
/// The pool runs in WAL mode, so a read transaction pins a snapshot without
/// blocking a writer, and a writer never blocks a reader. The mode persists
/// in the file; the `-wal` and `-shm` files sit beside it in the cache
/// directory.
pub async fn open_cache(layout: &ProvenanceLayout) -> anyhow::Result<SqlitePool> {
    std::fs::create_dir_all(layout.cache_dir())?;
    connect(cache_options(layout)?.create_if_missing(true)).await
}

/// Opens the cache database only when the file exists.
pub async fn open_existing_cache(layout: &ProvenanceLayout) -> anyhow::Result<SqlitePool> {
    connect(cache_options(layout)?.create_if_missing(false)).await
}

/// Opens the cache database as an immutable image, for a cache directory
/// this process cannot write: WAL cannot be read without a writable `-shm`
/// file, and an immutable open needs neither.
pub async fn open_immutable_cache(layout: &ProvenanceLayout) -> anyhow::Result<SqlitePool> {
    connect(
        cache_options(layout)?
            .create_if_missing(false)
            .read_only(true)
            .immutable(true),
    )
    .await
}

fn cache_options(layout: &ProvenanceLayout) -> anyhow::Result<SqliteConnectOptions> {
    Ok(
        SqliteConnectOptions::from_str(&format!("sqlite://{}", layout.cache_db_path()))?
            .journal_mode(SqliteJournalMode::Wal),
    )
}

/// Connects, retrying while the switch to WAL is refused as busy. The
/// switch needs an exclusive lock, and a build whose busy timeout does not
/// wait for it fails the first open of a DELETE-mode file another process
/// holds; once the file is WAL the switch is a no-op.
async fn connect(options: SqliteConnectOptions) -> anyhow::Result<SqlitePool> {
    let mut attempt = 0;
    loop {
        match SqlitePool::connect_with(options.clone()).await {
            Ok(pool) => return Ok(pool),
            Err(error) if is_busy(&error) && attempt < WAL_SWITCH_ATTEMPTS => {
                attempt += 1;
                tokio::time::sleep(WAL_SWITCH_PAUSE).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn is_busy(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database) => {
            database.code().as_deref() == Some("5") || database.message().contains("locked")
        }
        _ => false,
    }
}

pub(crate) fn serde_name<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    Ok(serde_json::to_value(value)?.as_str().unwrap().to_string())
}

#[cfg(test)]
pub(crate) mod tests;
