//! The owned cache connection and its completion.
//!
//! Every open returns a [`CacheConnection`], and the connection carries
//! its close with it: the ordered close lock, the detached task that
//! survives a cancelled or panicking owner, and the wait for the physical
//! `SQLite` teardown. Callers run work through [`CacheConnection::pool`]
//! and complete the connection with [`CacheConnection::close`]; no caller
//! holds a raw pool, and no caller selects between close variants.

use crate::layout::ProvenanceLayout;
use anyhow::Context;
use fs2::FileExt;
use provenance_macros::rule;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::time::Duration;

/// How often a close retries the close lock while another close holds it.
const CLOSE_LOCK_RETRY: Duration = Duration::from_millis(1);

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

/// One opened cache database, owned through completion.
///
/// The connection runs in WAL mode, so a read transaction pins a snapshot
/// without blocking a writer, and a writer never blocks a reader. The mode
/// persists in the file; the `-wal` and `-shm` files sit beside it in the
/// cache directory. The pool holds one connection: every caller reads and
/// writes one statement at a time, so one is enough, and it keeps the
/// close clean. `SQLx` returns a dropped connection through a spawned
/// task, so a pool that can grow can close two connections at once; the
/// ordered close prevents overlap between pools and the limit of one
/// prevents overlap within a pool.
pub struct CacheConnection {
    pool: Option<SqlitePool>,
    immutable: bool,
}

impl CacheConnection {
    /// Opens the pool behind this connection and records whether the open
    /// is an immutable image, which decides the shape of the completion.
    async fn connect(
        options: SqliteConnectOptions,
        immutable: bool,
        retry: WalSwitchRetry,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            pool: Some(connect(options, retry).await?),
            immutable,
        })
    }

    /// The pool, for the work this connection is open for. Completion
    /// stays with the connection: do not clone or close the pool here.
    pub(crate) const fn pool(&self) -> &SqlitePool {
        self.pool
            .as_ref()
            .expect("the pool stays until the completion takes it")
    }

    /// Wraps a pool built outside the open functions. Only the tests that
    /// pin a `SQLx` close race build one.
    #[cfg(test)]
    pub const fn from_pool(pool: SqlitePool) -> Self {
        Self {
            pool: Some(pool),
            immutable: false,
        }
    }

    /// Waits for the completion: the ordered close under the database's
    /// close lock, until the pool holds no connection and the physical
    /// `SQLite` teardown has finished. `SQLite` can then remove the -wal
    /// and -shm files. An immutable connection holds neither file and
    /// takes no lock.
    ///
    /// The completion runs on a task that owns the lock and the pool, so
    /// a caller cancelled mid-close, or one that panics there, cannot end
    /// the close early or release the lock before its teardown finishes.
    #[rule("rule_completed_read_leaves_no_wal_files")]
    pub async fn close(mut self) -> anyhow::Result<()> {
        let pool = self
            .pool
            .take()
            .expect("the pool stays until the completion takes it");
        let immutable = self.immutable;
        drop(self);
        tokio::spawn(complete(pool, immutable))
            .await
            .context("cache close task failed")?
    }

    /// Completes the connection after its work failed, then reports the
    /// work's outcome. A completion failure is reported instead: the
    /// caller must not keep a result whose close failed, and the close
    /// error says more about the cache than the work error does.
    pub(crate) async fn settle<T>(self, outcome: anyhow::Result<T>) -> anyhow::Result<T> {
        self.close().await?;
        outcome
    }

    /// Reports `error` after the completion of a connection whose work
    /// already failed. A completion failure is reported instead, for the
    /// reason [`CacheConnection::settle`] states.
    pub(crate) async fn close_reporting(self, error: anyhow::Error) -> anyhow::Error {
        match self.close().await {
            Ok(()) => error,
            Err(completion) => completion,
        }
    }
}

impl Drop for CacheConnection {
    /// Starts the completion on a detached task. Plain `Drop` cannot
    /// await the asynchronous teardown, and it must not skip it: a path
    /// that forgets to close, or that is unwound by a panic before its
    /// close, still ends with the ordered completion. A connection whose
    /// pool has already left for a waited close has nothing to do.
    /// Outside a runtime the pool drops as `SQLx` drops it.
    fn drop(&mut self) {
        let Some(pool) = self.pool.take() else {
            return;
        };
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(complete(pool, self.immutable));
        }
    }
}

/// Opens the cache database, creating it when absent.
pub async fn open_cache(layout: &ProvenanceLayout) -> anyhow::Result<CacheConnection> {
    open_cache_with(layout, WalSwitchRetry::default()).await
}

/// `open_cache` with the retry stated, so a test can shorten the waits.
pub async fn open_cache_with(
    layout: &ProvenanceLayout,
    retry: WalSwitchRetry,
) -> anyhow::Result<CacheConnection> {
    std::fs::create_dir_all(layout.cache_dir())?;
    CacheConnection::connect(cache_options(layout).create_if_missing(true), false, retry).await
}

/// Opens the cache database only when the file exists.
pub async fn open_existing_cache(layout: &ProvenanceLayout) -> anyhow::Result<CacheConnection> {
    CacheConnection::connect(
        cache_options(layout).create_if_missing(false),
        false,
        WalSwitchRetry::default(),
    )
    .await
}

/// Opens the cache database as an immutable image, for a cache directory
/// this process cannot write: WAL cannot be read without a writable `-shm`
/// file, and an immutable open needs neither.
pub async fn open_immutable_cache(layout: &ProvenanceLayout) -> anyhow::Result<CacheConnection> {
    CacheConnection::connect(
        cache_options(layout)
            .create_if_missing(false)
            .read_only(true)
            .immutable(true),
        true,
        WalSwitchRetry::default(),
    )
    .await
}

/// Opens the stored projection for reading. A permission failure selects
/// an immutable image of the stored projection.
#[rule("rule_read_only_checkout_answers_as_an_immutable_image")]
pub async fn open_stored_cache(layout: &ProvenanceLayout) -> anyhow::Result<CacheConnection> {
    match open_existing_cache(layout).await {
        Ok(connection) => Ok(connection),
        Err(error) if permission_failure(layout, &error) => open_immutable_cache(layout).await,
        Err(error) => Err(error),
    }
}

pub fn cache_options(layout: &ProvenanceLayout) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(layout.cache_db_path())
        .journal_mode(SqliteJournalMode::Wal)
}

/// Connects, retrying while the switch to WAL is refused as busy. The
/// switch needs an exclusive lock, and a build whose busy timeout does not
/// wait for it fails the first open of a DELETE-mode file another process
/// holds; once the file is WAL the switch is a no-op.
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

/// The completion of one connection, which always runs on a detached
/// task: the caller's future can end before the physical teardown does
/// without giving up the close order.
///
/// Two closes at the same moment can each fail their exclusive-lock attempt
/// before the other releases its shared lock, and both then skip cleanup.
/// The per-database close lock orders closes across independent pools. It
/// is released only after the pool holds no connection, which is after the
/// physical `SQLite` teardown has finished.
///
/// Never lock the database through another file descriptor: closing it can
/// release `SQLite`'s process-owned POSIX locks on active connections.
async fn complete(pool: SqlitePool, immutable: bool) -> anyhow::Result<()> {
    if immutable {
        close_until_empty(&pool).await;
        return Ok(());
    }
    let path = pool
        .connect_options()
        .get_filename()
        .with_extension("db.close.lock");
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
/// Every attempt uses a non-blocking file lock and returns at once, so waiting
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
        if error.raw_os_error() != fs2::lock_contended_error().raw_os_error() {
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

// Whether an open error says this process cannot write the cache: both
// PermissionDenied and ReadOnlyFilesystem are permission failures.
//
// SQLite omits the OS error. PERM and the permission-specific READONLY
// codes identify access failures. CANTOPEN and IOERR_SHMOPEN need an
// OS check because a missing database can also produce CANTOPEN.
pub fn permission_failure(layout: &ProvenanceLayout, error: &anyhow::Error) -> bool {
    if error.chain().any(|source| {
        source
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| {
                matches!(
                    error.kind(),
                    std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
                )
            })
    }) {
        return true;
    }
    let code = error
        .downcast_ref::<sqlx::Error>()
        .and_then(sqlx::Error::as_database_error)
        .and_then(sqlx::error::DatabaseError::code);
    match code.as_deref() {
        Some("3" | "8" | "520" | "1544") => true,
        Some("14" | "4618") => std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(layout.cache_db_path())
            .is_err_and(|error| {
                matches!(
                    error.kind(),
                    std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
                )
            }),
        _ => false,
    }
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn database_filename_is_not_parsed_as_a_url() {
        for root in [r"C:\repo?mode=ro", r"\\?\C:\repo", "/tmp/repo?mode=ro"] {
            let layout = ProvenanceLayout::new(root);
            let options = cache_options(&layout);
            assert_eq!(options.get_filename(), layout.cache_db_path().as_std_path());
        }
    }
}
