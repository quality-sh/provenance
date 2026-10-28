//! The completion of one cache connection and the caller's bounded wait
//! for it.
//!
//! The completion runs on a detached task that owns the close lock and the
//! pool until the physical `SQLite` teardown has finished. A caller waits
//! for that task only until its deadline; past the deadline the caller gets
//! an error that names the stage the completion is in, and the task goes
//! on, so a late teardown still runs in order.

use anyhow::Context;
use fs2::FileExt;
use provenance_macros::rule;
use sqlx::SqlitePool;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::JoinHandle;

/// How often a close retries the close lock while another close holds it.
const CLOSE_LOCK_RETRY: Duration = Duration::from_millis(1);

/// What one completion is waiting for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Stage {
    CloseLock(PathBuf),
    Teardown { connections: u32 },
}

impl Stage {
    /// The stage of a completion that has not started yet, shared between
    /// the completion task and the caller that waits for it.
    pub(super) fn shared(pool: &SqlitePool) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::Teardown {
            connections: pool.size(),
        }))
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CloseLock(path) => write!(f, "waiting for the close lock {}", path.display()),
            Self::Teardown { connections: 1 } => f.write_str("the pool still holds 1 connection"),
            Self::Teardown { connections } => {
                write!(f, "the pool still holds {connections} connections")
            }
        }
    }
}

fn enter(stage: &Mutex<Stage>, next: Stage) {
    *stage
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = next;
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
pub(super) async fn complete(
    pool: SqlitePool,
    immutable: bool,
    stage: Arc<Mutex<Stage>>,
) -> anyhow::Result<()> {
    if immutable {
        close_until_empty(&pool, &stage).await;
        return Ok(());
    }
    let path = pool
        .connect_options()
        .get_filename()
        .with_extension("db.close.lock");
    enter(&stage, Stage::CloseLock(path.clone()));
    let lock = match acquire_close_lock(&path).await {
        Ok(lock) => lock,
        Err(error) => {
            close_until_empty(&pool, &stage).await;
            return Err(error);
        }
    };
    close_until_empty(&pool, &stage).await;
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
async fn close_until_empty(pool: &SqlitePool, stage: &Mutex<Stage>) {
    loop {
        enter(
            stage,
            Stage::Teardown {
                connections: pool.size(),
            },
        );
        // Both SQLite closes can fail their exclusive-lock attempt before
        // either releases its shared lock. The close lock keeps this close
        // alone with the database.
        pool.close().await;
        if pool.size() == 0 {
            return;
        }
    }
}

/// Waits for the completion `task` until `deadline`. Past the deadline the
/// wait fails with the stage the completion is in; the task keeps the
/// close lock and the pool and goes on.
#[rule("rule_cache_completion_names_its_stage_at_the_deadline")]
pub(super) async fn within_deadline(
    task: JoinHandle<anyhow::Result<()>>,
    deadline: Duration,
    stage: Arc<Mutex<Stage>>,
) -> anyhow::Result<()> {
    if let Ok(joined) = tokio::time::timeout(deadline, task).await {
        return joined.context("cache close task failed")?;
    }
    let stage = stage
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    anyhow::bail!("cache close did not finish within {deadline:?}: {stage}")
}
