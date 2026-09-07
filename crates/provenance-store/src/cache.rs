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
    catch_up_state, materialize_empty_state, materialize_state, scope_ids, unit_digest, units_for,
    CatchUpReport, Unit, UnitHashError,
};
pub use prime::*;
pub use projection_digest::{
    family_content_digests, revision_digest, revision_digest_from_stored_rows, FamilyContentDigest,
};
pub use projection_families::ProjectionFamily;
pub use traceability::*;

use crate::layout::ProvenanceLayout;
use anyhow::Context;
use fs2::FileExt;
use provenance_macros::rule;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{ConnectOptions, SqlitePool};
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

/// How often a close retries the close lock while another close holds it.
const CLOSE_LOCK_RETRY: Duration = Duration::from_millis(1);

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
/// `SQLx` returns a dropped connection through a spawned task. A pool that
/// can grow can thus close two connections at once. `close_cache` orders
/// closes across pools; the limit of one also prevents overlap within a pool.
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

/// Closes every cache connection before a read returns, so `SQLite` can remove
/// the -wal and -shm files when the last connection closes.
///
/// Two closes at the same moment can each fail their exclusive-lock attempt
/// before the other releases its shared lock, and both then skip cleanup.
/// The per-database close lock orders closes across independent pools, and
/// the lock belongs to a detached close task rather than to the caller:
///
/// - The wait for the lock is a non-blocking attempt on the calling worker,
///   retried after a sleep. Publication waiters can occupy every blocking
///   worker while this read still holds the publication guard, so a lock
///   wait that needed a blocking worker could never run.
/// - A reader cancelled mid-close, or one that panics there, cannot release
///   the lock early. The close task owns the lock and finishes the close
///   after the reader is gone, so the order does not depend on the reader's
///   future.
/// - The lock is released only after the pool holds no connection, which is
///   after the physical `SQLite` teardown has finished.
///
/// Never lock the database through another file descriptor: closing it can
/// release `SQLite`'s process-owned POSIX locks on active connections.
#[rule("rule_completed_read_leaves_no_wal_files")]
pub(crate) async fn close_cache(pool: &SqlitePool) -> anyhow::Result<()> {
    let options = pool.connect_options();
    // Immutable reads do not use the -wal and -shm files or need a lock file.
    let immutable = options
        .to_url_lossy()
        .query_pairs()
        .any(|(key, value)| key == "immutable" && value == "true");
    if immutable {
        close_until_empty(pool).await;
        return Ok(());
    }
    let path = options.get_filename().with_extension("db.close.lock");
    tokio::spawn(close_in_order(pool.clone(), path))
        .await
        .context("cache close task failed")?
}

/// Closes one pool under the database's close lock.
///
/// The task owns the lock file, so the caller's future can end before the
/// physical teardown does without giving up the close order. A lock that
/// cannot be acquired is reported after the pool closes.
async fn close_in_order(pool: SqlitePool, path: PathBuf) -> anyhow::Result<()> {
    let lock = match acquire_close_lock(&path).await {
        Ok(lock) => lock,
        Err(error) => {
            close_until_empty(&pool).await;
            return Err(error);
        }
    };
    close_until_empty(&pool).await;
    drop(lock);
    Ok(())
}

/// Opens the close lock file and waits until this close holds it.
///
/// Every attempt is a non-blocking `flock` that returns at once, so waiting
/// for the lock never occupies a blocking worker; publication waiters can
/// hold them all while this close waits.
async fn acquire_close_lock(path: &Path) -> anyhow::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("open cache close lock {}", path.display()))?;
    while let Err(error) = file.try_lock_exclusive() {
        if error.kind() != ErrorKind::WouldBlock {
            return Err(anyhow::Error::new(error)
                .context(format!("acquire cache close lock {}", path.display())));
        }
        tokio::time::sleep(CLOSE_LOCK_RETRY).await;
    }
    Ok(file)
}

/// Closes the pool and waits until it holds no connection.
///
/// `SQLx 0.8` can return from close with a connection that reached the idle
/// queue after its last drain; the closed pool cannot grow. A pool with no
/// connection has dropped every connection state, which is the moment the
/// last `SQLite` handle is gone, so the close lock is held over the whole
/// physical teardown.
async fn close_until_empty(pool: &SqlitePool) {
    loop {
        // Both SQLite closes can fail their exclusive-lock attempt before
        // either releases its shared lock. The close lock keeps this close
        // alone with the database.
        pool.close().await;
        if pool.size() == 0 {
            return;
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
