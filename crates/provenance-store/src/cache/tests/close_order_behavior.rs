//! The close lock orders closes across pools, and a close owns that lock
//! from before its pool closes until the physical teardown finishes. The
//! tests here hold the two properties that order depends on: the wait for
//! the lock never needs a blocking worker, and the lock outlives a reader
//! that is cancelled or panics mid-close.

use super::super::*;
use super::fixtures::empty_layout;
use super::wal_behavior::wal_files;
use crate::cache::CacheConnection;
use crate::layout::ProvenanceLayout;
use crate::operations::read_policy::ReadPolicy;
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tokio::sync::oneshot;

/// How long a close may take once nothing blocks it.
const CLOSE_LIMIT: Duration = Duration::from_secs(10);

/// How long a close that must stay behind a paused one is watched to prove
/// it cannot pass.
const BLOCKED_CLOSE_PROOF: Duration = Duration::from_millis(300);

/// Builds a runtime whose blocking pool has exactly `workers` workers, so
/// the test can fill them all with publication waiters.
fn small_blocking_pool(workers: usize) -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(workers)
        .build()
        .expect("the test runtime must build")
}

/// The handle of one competing publication call blocked on the publication
/// lock that the test's read holds.
type Waiter = tokio::task::JoinHandle<anyhow::Result<()>>;

/// Starts one competing publication call on the blocking pool and returns
/// once it has entered, so the caller knows a worker is held.
fn start_publication_waiter(
    layout: &ProvenanceLayout,
    entered: &std::sync::mpsc::Sender<()>,
) -> Waiter {
    let layout = layout.clone();
    let entered = entered.clone();
    tokio::task::spawn_blocking(move || {
        entered
            .send(())
            .expect("the test must still be waiting for this waiter");
        crate::publication::with_repository_publication(&layout, || Ok(()))
    })
}

/// Waits for every publication waiter to finish, which needs the test's
/// publication guard released first.
async fn join_waiters(waiters: &Mutex<Vec<Waiter>>) {
    let handles = waiters.lock().unwrap().drain(..).collect::<Vec<_>>();
    for handle in handles {
        tokio::time::timeout(CLOSE_LIMIT, handle)
            .await
            .expect("each publication waiter must finish once the guard is released")
            .expect("the publication waiter task must not fail")
            .expect("the publication section must succeed");
    }
}

/// A read whose freshness step fails while the only blocking worker waits
/// for the publication guard must still finish: its close waits for the
/// close lock without a worker, or the read would wait for itself to
/// release the guard it needs to give up.
#[test]
fn a_read_closes_while_the_only_blocking_worker_waits_for_its_guard() {
    let runtime = small_blocking_pool(1);
    runtime.block_on(async {
        let (_dir, layout, scope) = empty_layout();
        let waiters = Arc::new(Mutex::new(Vec::new()));
        let waiters_in_probe = Arc::clone(&waiters);
        let waiter_layout = layout.clone();
        crate::test_probes::arm("run_migrations_under_guard", move || {
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            let waiter = start_publication_waiter(&waiter_layout, &entered_tx);
            entered_rx
                .recv_timeout(CLOSE_LIMIT)
                .expect("the publication waiter must reach the lock");
            waiters_in_probe.lock().unwrap().push(waiter);
            anyhow::bail!("the freshness step fails while another publication waits")
        });
        let answer = tokio::time::timeout(
            CLOSE_LIMIT,
            crate::operations::reader::answer(layout.root(), &scope, ReadPolicy::default(), |_| {
                Box::pin(async { Ok(1usize) })
            }),
        )
        .await
        .expect("the read must finish while the only blocking worker waits for its guard");
        crate::test_probes::disarm("run_migrations_under_guard");
        assert!(
            answer.is_err(),
            "the injected freshness failure must surface: {answer:?}"
        );
        join_waiters(&waiters).await;
    });
    runtime.shutdown_timeout(Duration::from_secs(2));
}

/// A valid materialization whose close runs while every blocking worker
/// waits for its publication guard must finish for the same reason.
#[test]
fn a_materialization_closes_while_every_blocking_worker_waits_for_its_guard() {
    const WORKERS: usize = 4;
    let runtime = small_blocking_pool(WORKERS);
    runtime.block_on(async {
        let (_dir, layout, _scope) = empty_layout();
        let waiters = Arc::new(Mutex::new(Vec::new()));
        let waiters_in_probe = Arc::clone(&waiters);
        let waiter_layout = layout.clone();
        crate::test_probes::arm("run_migrations_under_guard", move || {
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            for _ in 0..WORKERS {
                let waiter = start_publication_waiter(&waiter_layout, &entered_tx);
                waiters_in_probe.lock().unwrap().push(waiter);
            }
            for _ in 0..WORKERS {
                entered_rx
                    .recv_timeout(CLOSE_LIMIT)
                    .expect("each publication waiter must reach the lock");
            }
            Ok(())
        });
        let report = tokio::time::timeout(CLOSE_LIMIT, materialize_empty_state(&layout))
            .await
            .expect(
                "the materialization must finish while every blocking worker waits for its guard",
            )
            .expect("the materialization itself must succeed");
        crate::test_probes::disarm("run_migrations_under_guard");
        assert!(!report.migrations_applied.is_empty());
        join_waiters(&waiters).await;
    });
    runtime.shutdown_timeout(Duration::from_secs(2));
}

struct Release(Arc<(Mutex<bool>, Condvar)>);

impl Drop for Release {
    fn drop(&mut self) {
        let (mutex, done) = &*self.0;
        *mutex.lock().unwrap() = true;
        done.notify_all();
    }
}

struct PauseOnDrop {
    signal: Option<oneshot::Sender<()>>,
    gate: Arc<(Mutex<bool>, Condvar)>,
}

impl Drop for PauseOnDrop {
    fn drop(&mut self) {
        if let Some(signal) = self.signal.take() {
            let _ = signal.send(());
        }
        let (mutex, done) = &*self.gate;
        let mut released = mutex.lock().unwrap();
        while !*released {
            released = done.wait(released).unwrap();
        }
        drop(released);
    }
}

/// Hands the pool a connection whose physical close pauses until the
/// returned `Release` is dropped. `SQLx` removes a progress handler while
/// dropping the connection state, before the `SQLite` handle drops, so the
/// destructor of this handler pauses the teardown at that exact point and
/// reports that the pause has begun.
async fn pause_physical_close(pool: &CacheConnection) -> (Release, oneshot::Receiver<()>) {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let (signal, started) = oneshot::channel();
    let pause = PauseOnDrop {
        signal: Some(signal),
        gate: Arc::clone(&gate),
    };
    let mut connection = pool
        .pool()
        .acquire()
        .await
        .expect("the pool must hand out a connection");
    let mut handle = connection
        .lock_handle()
        .await
        .expect("the connection handle must lock");
    handle.set_progress_handler(1, move || {
        std::hint::black_box(&pause);
        true
    });
    drop(handle);
    drop(connection);
    (Release(gate), started)
}

/// Two pools over one database, each holding one open connection, so the
/// second close has teardown of its own to order behind the first.
async fn two_pools_over_one_database(
    layout: &ProvenanceLayout,
) -> (CacheConnection, CacheConnection) {
    let first = open_cache(layout).await.expect("the first pool must open");
    sqlx::query("CREATE TABLE sample(value INTEGER)")
        .execute(first.pool())
        .await
        .expect("the table must create");
    sqlx::query("INSERT INTO sample VALUES (1)")
        .execute(first.pool())
        .await
        .expect("the row must insert");
    let second = open_cache(layout)
        .await
        .expect("the second pool must open over the same database");
    sqlx::query("SELECT * FROM sample")
        .fetch_all(second.pool())
        .await
        .expect("the second pool must read");
    (first, second)
}

/// Opens the close lock file from outside, as a second close would. The
/// caller may only use it once the close under test has created the file.
fn open_close_lock(layout: &ProvenanceLayout) -> File {
    let path = layout.cache_db_path().with_extension("db.close.lock");
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("the close under test must have created the lock file")
}

/// Waits until the close that owns the lock has released it, and lets go
/// again so a later close can take it.
async fn wait_for_close_lock(outside: &File) {
    tokio::time::timeout(CLOSE_LIMIT, async {
        loop {
            if outside.try_lock_exclusive().is_ok() {
                let _ = outside.unlock();
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the close lock must be released once the teardown finishes");
}

/// Waits until no -wal or -shm file remains beside the database.
async fn wait_for_wal_files(layout: &ProvenanceLayout) {
    tokio::time::timeout(CLOSE_LIMIT, async {
        loop {
            if wal_files(layout).is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the -wal and -shm files must be gone once every close finishes");
}

/// A close owned by a reader that is cancelled mid-close must keep the
/// close lock until its physical teardown finishes, and no later close may
/// pass it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reader_cancelled_mid_close_keeps_the_close_order() {
    let (_dir, layout, _scope) = empty_layout();
    let (first, second) = two_pools_over_one_database(&layout).await;
    let (release, started) = pause_physical_close(&first).await;
    let task = tokio::spawn(async move {
        first
            .close()
            .await
            .expect("the close must succeed once the paused teardown resumes");
    });
    tokio::time::timeout(CLOSE_LIMIT, started)
        .await
        .expect("the close must reach its teardown while the pause holds")
        .expect("the pause must report before its destructor blocks");
    let outside = open_close_lock(&layout);
    assert!(
        outside.try_lock_exclusive().is_err(),
        "the close must own the close lock while its teardown runs"
    );
    task.abort();
    let error = task
        .await
        .expect_err("the cancelled reader task must not return a result");
    assert!(error.is_cancelled(), "the reader must end cancelled");
    assert!(
        outside.try_lock_exclusive().is_err(),
        "a cancelled reader must not release the close lock before its teardown finishes"
    );
    let passing_close = tokio::time::timeout(BLOCKED_CLOSE_PROOF, second.close()).await;
    assert!(
        passing_close.is_err(),
        "no close may pass the close that owns the lock"
    );
    drop(release);
    wait_for_close_lock(&outside).await;
    wait_for_wal_files(&layout).await;
}

/// A close owned by a reader that panics mid-close must keep the close lock
/// until its physical teardown finishes, and no later close may pass it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reader_panicking_mid_close_keeps_the_close_order() {
    let (_dir, layout, _scope) = empty_layout();
    let (first, second) = two_pools_over_one_database(&layout).await;
    let (release, started) = pause_physical_close(&first).await;
    let (panic_tx, panic_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        tokio::select! {
            result = first.close() => {
                result.expect("the close must succeed once the paused teardown resumes");
            }
            _ = panic_rx => panic!("the reader panics while its close is pending"),
        }
    });
    tokio::time::timeout(CLOSE_LIMIT, started)
        .await
        .expect("the close must reach its teardown while the pause holds")
        .expect("the pause must report before its destructor blocks");
    let outside = open_close_lock(&layout);
    assert!(
        outside.try_lock_exclusive().is_err(),
        "the close must own the close lock while its teardown runs"
    );
    panic_tx
        .send(())
        .expect("the reader task must still be waiting");
    let error = task
        .await
        .expect_err("the panicking reader task must not return a result");
    assert!(
        error.is_panic(),
        "the reader must end in a panic: {error:?}"
    );
    assert!(
        outside.try_lock_exclusive().is_err(),
        "a panicking reader must not release the close lock before its teardown finishes"
    );
    let passing_close = tokio::time::timeout(BLOCKED_CLOSE_PROOF, second.close()).await;
    assert!(
        passing_close.is_err(),
        "no close may pass the close that owns the lock"
    );
    drop(release);
    wait_for_close_lock(&outside).await;
    wait_for_wal_files(&layout).await;
}
