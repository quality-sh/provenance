//! A close that is not complete at its deadline fails and names the stage
//! of the wait. The completion keeps the close lock and goes on.
use super::close_cancellation::pause_physical_close;
use super::wal_behavior::wal_files;
use crate::cache::{open_cache, CacheConnection};
use crate::layout::ProvenanceLayout;
use fs2::FileExt;
use provenance_macros::verifies;
use std::fs::{File, OpenOptions};
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_millis(300);
const LIMIT: Duration = Duration::from_secs(5);

async fn written(layout: &ProvenanceLayout) -> CacheConnection {
    let connection = open_cache(layout).await.unwrap();
    sqlx::query("CREATE TABLE sample(value INTEGER)")
        .execute(connection.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO sample VALUES (1)")
        .execute(connection.pool())
        .await
        .unwrap();
    connection
}

fn close_lock(layout: &ProvenanceLayout) -> File {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(layout.cache_db_path().with_extension("db.close.lock"))
        .unwrap()
}

/// Waits until the completion that outlived its caller releases the close
/// lock and `SQLite` has removed the -wal and -shm files.
async fn late_completion_finishes(layout: &ProvenanceLayout, lock: &File) {
    let started = Instant::now();
    loop {
        let removed = wal_files(layout).is_empty();
        if lock.try_lock_exclusive().is_ok() {
            FileExt::unlock(lock).unwrap();
            if removed {
                return;
            }
        }
        assert!(
            started.elapsed() < LIMIT,
            "the completion kept the lock or left the -wal and -shm files"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn temporary_layout() -> (tempfile::TempDir, ProvenanceLayout) {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(
        camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap(),
    );
    (dir, layout)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[verifies("rule_cache_completion_names_its_stage_at_the_deadline", examples)]
async fn a_paused_teardown_fails_the_close_naming_the_pool() {
    let (_dir, layout) = temporary_layout();
    let connection = written(&layout).await;
    let (release, _started) = pause_physical_close(&connection).await;

    let error = tokio::time::timeout(LIMIT, connection.completing_within(DEADLINE).close())
        .await
        .expect("the close ends at its deadline")
        .expect_err("a paused teardown fails the close");
    let message = format!("{error:#}");
    assert!(
        message.contains("cache close did not finish within 300ms: the pool still holds"),
        "{message}"
    );

    let lock = close_lock(&layout);
    assert!(
        lock.try_lock_exclusive().is_err(),
        "the completion keeps the close lock past the deadline"
    );
    drop(release);
    late_completion_finishes(&layout, &lock).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[verifies("rule_cache_completion_names_its_stage_at_the_deadline", examples)]
async fn a_held_close_lock_fails_the_close_naming_the_lock() {
    let (_dir, layout) = temporary_layout();
    let connection = written(&layout).await;
    let lock = close_lock(&layout);
    lock.lock_exclusive().unwrap();

    let error = tokio::time::timeout(LIMIT, connection.completing_within(DEADLINE).close())
        .await
        .expect("the close ends at its deadline")
        .expect_err("a held close lock fails the close");
    let path = layout.cache_db_path().with_extension("db.close.lock");
    assert_eq!(
        format!("{error:#}"),
        format!("cache close did not finish within 300ms: waiting for the close lock {path}")
    );

    FileExt::unlock(&lock).unwrap();
    late_completion_finishes(&layout, &lock).await;
}
