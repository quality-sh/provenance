mod gaps;
mod health;
mod impact;
mod materialize;
mod prime;
mod projection_digest;
mod projection_families;
pub mod read;
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
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MaterializeReport {
    pub records_loaded: u64,
    pub migrations_applied: Vec<String>,
}

/// How a connect waits while another process holds a DELETE-mode file
/// open across the switch to WAL.
///
/// Each attempt waits up to `busy_timeout` inside `SQLite`, then the connect
/// fails as busy and the next attempt starts after `pause`, until
/// `deadline` of wall time has passed since the first attempt. A caller
/// that opens the file under the publication guard therefore holds the
/// guard for at most `deadline` plus one `busy_timeout` in the legacy-file
/// case; with the defaults that is fifteen seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalSwitchRetry {
    pub busy_timeout: Duration,
    pub pause: Duration,
    pub deadline: Duration,
}

impl Default for WalSwitchRetry {
    fn default() -> Self {
        Self {
            busy_timeout: Duration::from_secs(5),
            pause: Duration::from_millis(100),
            deadline: Duration::from_secs(10),
        }
    }
}

/// Opens the cache database, creating it when absent.
///
/// The pool runs in WAL mode, so a read transaction pins a snapshot without
/// blocking a writer, and a writer never blocks a reader. The mode persists
/// in the file; the `-wal` and `-shm` files sit beside it in the cache
/// directory.
pub async fn open_cache(layout: &ProvenanceLayout) -> anyhow::Result<SqlitePool> {
    open_cache_with(layout, WalSwitchRetry::default()).await
}

/// `open_cache` with the retry stated, so a test can shorten the waits.
pub async fn open_cache_with(
    layout: &ProvenanceLayout,
    retry: WalSwitchRetry,
) -> anyhow::Result<SqlitePool> {
    std::fs::create_dir_all(layout.cache_dir())?;
    connect(cache_options(layout)?.create_if_missing(true), retry).await
}

/// Opens the cache database only when the file exists.
pub async fn open_existing_cache(layout: &ProvenanceLayout) -> anyhow::Result<SqlitePool> {
    connect(
        cache_options(layout)?.create_if_missing(false),
        WalSwitchRetry::default(),
    )
    .await
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
        WalSwitchRetry::default(),
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
///
/// The pool holds one connection. Every caller reads and writes one
/// statement at a time, so one is enough, and it keeps the close clean:
/// sqlx returns a dropped connection through a spawned task, so a pool
/// that may grow opens a second connection for the next statement, and at
/// `close` the two `sqlite3_close` calls overlap on two worker threads.
/// The last of them cannot take the exclusive lock, and `SQLite` then skips
/// the checkpoint that removes the `-wal` and `-shm` files. A pool of one
/// closes once, and the files go with it.
async fn connect(
    options: SqliteConnectOptions,
    retry: WalSwitchRetry,
) -> anyhow::Result<SqlitePool> {
    let options = options.busy_timeout(retry.busy_timeout);
    let started = std::time::Instant::now();
    loop {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await;
        match pool {
            Ok(pool) => return Ok(pool),
            Err(error) if is_busy(&error) && started.elapsed() < retry.deadline => {
                tokio::time::sleep(retry.pause).await;
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

/// A quoted SQL identifier: `key`, `field`, `before`, and `after` are
/// column names and SQL keywords both.
pub(crate) fn quoted(identifier: &str) -> String {
    format!("\"{identifier}\"")
}

#[cfg(test)]
pub(crate) mod tests;
