//! Serial and revision behavior of the hash-based catch-up.
//!
//! No journal, no head record, no shared sequence space: the serial is
//! stored serial plus one whenever a pass changes something, and a pass
//! that changes nothing commits no revision at all. Serials are scoped to
//! the projection instance; a lost database restarts at one under a fresh
//! instance id.

use super::super::*;
use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::fixtures::*;
use crate::state_store::StateStore;

/// Deletes the database file the way a user would, tolerating one
/// Windows-specific delay: sqlx closes `SQLite` connections on a worker
/// thread, and `Pool::close()` can return before the OS releases the file
/// handle, so a delete that follows a pass immediately can meet a sharing
/// violation for a few milliseconds. The wait is bounded and accepts only
/// that error; any other failure is a real one.
fn remove_database_file(layout: &crate::layout::ProvenanceLayout) {
    let path = layout.cache_db_path();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match std::fs::remove_file(&path) {
            Ok(()) => return,
            Err(error)
                if error.raw_os_error() == Some(32) && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(error) => panic!("remove {path}: {error}"),
        }
    }
}

pub(super) async fn latest_revision(pool: &sqlx::SqlitePool) -> (i64, String) {
    sqlx::query_as("SELECT serial, digest FROM projection_revision ORDER BY serial DESC LIMIT 1")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn revision_count(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM projection_revision")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn instance_id(pool: &sqlx::SqlitePool) -> String {
    sqlx::query_scalar("SELECT instance_id FROM projection_instance")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_pass_that_changes_nothing_commits_no_revision() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let before = latest_revision(&pool).await;
    let revisions_before = revision_count(&pool).await;
    pool.close().await;

    let report = catch_up_state(&layout).await.unwrap();
    assert_eq!(report.rows_written, 0);

    let pool = open_cache(&layout).await.unwrap();
    assert_eq!(
        latest_revision(&pool).await,
        before,
        "a no-op pass keeps serial and digest"
    );
    assert_eq!(
        revision_count(&pool).await,
        revisions_before,
        "a no-op pass commits no revision row"
    );
}

#[tokio::test]
async fn a_writer_commit_reaches_the_projection_on_the_next_pass() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let (serial_before, _) = latest_revision(&pool).await;
    pool.close().await;

    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_after_stamp");

    let report = catch_up_state(&layout).await.unwrap();
    assert!(!report.rebuilt);
    assert_eq!(
        report.serial,
        serial_before + 1,
        "a change bumps the serial once"
    );
    assert!(report.rows_written > 0);
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn two_passes_over_unchanged_state_write_nothing_twice() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_once");
    let first = catch_up_state(&layout).await.unwrap();
    assert!(first.rows_written > 0);

    let second = catch_up_state(&layout).await.unwrap();
    let third = catch_up_state(&layout).await.unwrap();
    for report in [&second, &third] {
        assert_eq!(report.rows_written, 0, "{report:?}");
        assert_eq!(report.families_rederived, 0, "{report:?}");
        assert_eq!(
            report.serial, first.serial,
            "no revision after a no-op pass"
        );
        assert_eq!(report.digest, first.digest);
    }
}

#[tokio::test]
async fn a_crash_after_commit_leaves_consistent_state_and_the_next_pass_finds_nothing() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_before_crash");

    crate::test_probes::crash_at("catch_up_after_commit");
    let error = catch_up_state(&layout).await.unwrap_err();
    crate::test_probes::disarm("catch_up_after_commit");
    assert!(error.to_string().contains("injected crash"), "{error}");

    let pool = open_cache(&layout).await.unwrap();
    let (committed_serial, _) = latest_revision(&pool).await;
    pool.close().await;

    let report = catch_up_state(&layout).await.unwrap();
    assert_eq!(
        report.rows_written, 0,
        "the crash lost nothing after commit"
    );
    assert_eq!(report.serial, committed_serial, "nothing to commit again");
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn a_crash_before_commit_leaves_the_previous_stamp_readable() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let before = latest_revision(&pool).await;
    pool.close().await;
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_uncommitted");

    crate::test_probes::crash_at("catch_up_before_commit");
    let error = catch_up_state(&layout).await.unwrap_err();
    crate::test_probes::disarm("catch_up_before_commit");
    assert!(error.to_string().contains("injected crash"), "{error}");

    let pool = open_cache(&layout).await.unwrap();
    assert_eq!(latest_revision(&pool).await, before, "the old stamp stands");
    pool.close().await;
    let report = catch_up_state(&layout).await.unwrap();
    assert_eq!(report.serial, before.0 + 1);
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_concurrent_rebuild_and_catch_up_yield_one_serial_progression() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_contended");

    let rebuild_layout = layout.clone();
    let catch_up_layout = layout.clone();
    let rebuild = tokio::spawn(async move { materialize_state(&rebuild_layout).await });
    let catch_up = tokio::spawn(async move { catch_up_state(&catch_up_layout).await });
    rebuild.await.unwrap().unwrap();
    catch_up.await.unwrap().unwrap();

    let pool = open_cache(&layout).await.unwrap();
    let serials: Vec<i64> =
        sqlx::query_scalar("SELECT serial FROM projection_revision ORDER BY serial")
            .fetch_all(&pool)
            .await
            .unwrap();
    let expected: Vec<i64> = (1..=i64::try_from(serials.len()).unwrap()).collect();
    assert_eq!(serials, expected, "one gapless, duplicate-free progression");
    pool.close().await;
    assert_catch_up_equals_rebuild(&layout).await;
}

#[tokio::test]
async fn total_cache_loss_restarts_the_serial_at_one_in_a_fresh_instance() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_second_serial");
    catch_up_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let (serial, digest) = latest_revision(&pool).await;
    let first_instance = instance_id(&pool).await;
    pool.close().await;
    assert_eq!(serial, 2);

    std::fs::remove_dir_all(layout.cache_dir()).unwrap();

    let report = catch_up_state(&layout).await.unwrap();
    assert!(report.rebuilt);
    assert_eq!(report.serial, 1, "a fresh instance starts at one");
    assert_eq!(report.digest, digest, "same canonical bytes, same digest");
    let pool = open_cache(&layout).await.unwrap();
    assert_ne!(instance_id(&pool).await, first_instance);
}

#[tokio::test]
async fn a_lost_database_with_canonical_state_intact_rebuilds_at_one() {
    let (_dir, layout, scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_survives_db_loss");
    catch_up_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let (serial_before_loss, _) = latest_revision(&pool).await;
    pool.close().await;
    assert_eq!(
        serial_before_loss, 2,
        "the contrast the restart is measured against"
    );

    remove_database_file(&layout);

    let report = catch_up_state(&layout).await.unwrap();
    assert!(report.rebuilt);
    assert_eq!(report.serial, 1);
    assert_catch_up_equals_rebuild(&layout).await;
}
