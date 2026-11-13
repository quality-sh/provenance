//! What a read does to the read database file today, under each freshness
//! policy. These tests have no Rule. They hold the file behaviour while the
//! read database moves behind a port, so the move changes no answer, no
//! refusal text and no file beside the database.

use super::super::comparison::test_stores::{self, TestStore};
use super::get_through;
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::publication::publication_guard;
use provenance_core::protocol::StampPolicy;
use provenance_core::RequirementStatus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn side_files(store: &TestStore) -> Vec<String> {
    let database = store.layout().cache_db_path();
    ["-wal", "-shm", "-journal"]
        .into_iter()
        .map(|suffix| format!("{database}{suffix}"))
        .filter(|path| std::path::Path::new(path).exists())
        .collect()
}

#[tokio::test]
async fn each_policy_answers_at_one_serial_and_leaves_no_side_files() {
    let store = test_stores::seeded_queries();
    assert!(!store.layout().cache_db_path().exists());

    let caught_up = get_through(&store, ReadPolicy::default()).await.unwrap();
    assert!(caught_up.result.found);
    assert_eq!(caught_up.stamp.policy, StampPolicy::CatchUp);
    assert!(caught_up.freshness_error.is_none());
    assert!(
        store.layout().cache_db_path().exists(),
        "catch-up creates it"
    );
    assert_eq!(side_files(&store), Vec::<String>::new());

    for (policy, word) in [
        (FreshnessPolicy::AnnotateOnly, StampPolicy::AnnotateOnly),
        (FreshnessPolicy::RefuseStale, StampPolicy::RefuseStale),
        (FreshnessPolicy::CatchUp, StampPolicy::CatchUp),
    ] {
        let stamped = get_through(&store, ReadPolicy::with_freshness(policy))
            .await
            .unwrap();
        assert_eq!(stamped.stamp.policy, word);
        assert_eq!(stamped.stamp.serial, caught_up.stamp.serial);
        assert_eq!(stamped.stamp.digest, caught_up.stamp.digest);
        assert_eq!(
            serde_json::to_value(&stamped.result).unwrap(),
            serde_json::to_value(&caught_up.result).unwrap()
        );
        assert_eq!(side_files(&store), Vec::<String>::new(), "{word:?}");
    }
}

#[tokio::test]
async fn a_read_with_no_database_names_the_file_and_creates_nothing() {
    let store = test_stores::seeded_queries();
    let database = store.layout().cache_db_path();
    for policy in [FreshnessPolicy::AnnotateOnly, FreshnessPolicy::RefuseStale] {
        let refused = get_through(&store, ReadPolicy::with_freshness(policy))
            .await
            .unwrap_err()
            .to_string();
        let expected =
            format!("no projection revision in {database}; run `provenance materialize` (");
        assert!(refused.starts_with(&expected), "{policy:?}: {refused}");
        assert!(!database.exists(), "{policy:?}");
    }
}

#[tokio::test]
async fn a_stale_projection_refuses_with_the_file_and_the_moved_unit() {
    let store = test_stores::seeded_queries();
    let caught_up = get_through(&store, ReadPolicy::default()).await.unwrap();
    crate::cache::tests::fixtures::create_requirement(
        &store.state_store(),
        &store.scope,
        "req_after_the_read",
        provenance_core::RequirementStatus::Active,
    );
    let refused = get_through(
        &store,
        ReadPolicy::with_freshness(FreshnessPolicy::RefuseStale),
    )
    .await
    .unwrap_err()
    .to_string();
    let expected = format!(
        "refuse_stale: the projection in {} at serial {} (digest {}, instance ",
        store.layout().cache_db_path(),
        caught_up.stamp.serial,
        caught_up.stamp.digest
    );
    assert!(refused.starts_with(&expected), "{refused}");
    assert!(refused.contains("; moved: "), "{refused}");
    assert_eq!(side_files(&store), Vec::<String>::new());
}

/// Holds the close lock of the database, as another close does, until
/// `release` passes. The flag turns on just before the lock is freed.
fn hold_the_close_lock(
    store: &TestStore,
    release: std::time::Duration,
) -> (std::path::PathBuf, Arc<AtomicBool>) {
    use fs2::FileExt;
    let database = store.layout().cache_db_path();
    std::fs::create_dir_all(database.parent().unwrap()).unwrap();
    let path = database.with_extension("db.close.lock").into_std_path_buf();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .unwrap();
    lock.lock_exclusive().unwrap();
    let released = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&released);
    tokio::spawn(async move {
        tokio::time::sleep(release).await;
        flag.store(true, Ordering::SeqCst);
        FileExt::unlock(&lock).unwrap();
    });
    (path, released)
}

/// The catch-up and the read share one connection, so a close that cannot
/// finish fails the read with the close error, after the catch-up kept its
/// revision. The close does not become a failed catch-up beside an answer.
#[tokio::test(flavor = "multi_thread")]
async fn a_close_that_cannot_finish_after_a_committed_catch_up_fails_the_read() {
    let store = test_stores::seeded_queries();
    // Past the completion deadline, and before a second close would reach
    // its own.
    let (lock, _) = hold_the_close_lock(&store, Duration::from_secs(13));

    let failed = get_through(&store, ReadPolicy::default())
        .await
        .unwrap_err();
    let timed_out = failed
        .chain()
        .find_map(|cause| cause.downcast_ref::<crate::cache::CompletionTimedOut>())
        .unwrap_or_else(|| panic!("not a close failure: {failed:#}"));
    assert!(
        matches!(
            &timed_out.stage,
            crate::cache::completion::Stage::CloseLock(path) if path.ends_with(lock.file_name().unwrap())
        ),
        "{failed:#}"
    );
    let text = failed.to_string();
    assert!(
        text.starts_with("cache close did not finish within 10s: waiting for the close lock ")
            && text.ends_with("db.close.lock"),
        "{text}"
    );

    tokio::time::sleep(Duration::from_secs(4)).await;
    // No database was there before the read, so only its catch-up made
    // the revision that this read answers at.
    get_through(
        &store,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
    )
    .await
    .expect("the catch-up kept its revision");
}

/// A store whose catch-up refuses: its one Rule names no Requirement, after
/// a read that caught up while the Rule was whole.
async fn a_catch_up_that_refuses() -> (TestStore, provenance_core::protocol::Stamp) {
    let store = test_stores::seeded_queries();
    crate::cache::tests::fixtures::create_requirement(
        &store.state_store(),
        &store.scope,
        "req_rule_anchor",
        RequirementStatus::Active,
    );
    crate::cache::tests::fixtures::create_rule_of(
        &store.state_store(),
        &store.scope,
        "rule_anchored",
        "req_rule_anchor",
    );
    let healthy = get_through(&store, ReadPolicy::default()).await.unwrap();
    crate::cache::tests::fixtures::rewrite_records(
        &crate::shards::rules_path(&store.layout(), &store.scope),
        |record| record["requirement_ids"] = serde_json::json!([]),
    );
    (store, healthy.stamp)
}

/// A catch-up that fails holds the publication guard until its connection
/// has closed. The next writer then proceeds, and the read database keeps
/// the revision that it had.
#[tokio::test(flavor = "multi_thread")]
async fn a_failed_catch_up_frees_the_guard_after_its_close_and_keeps_its_revision() {
    let (store, healthy) = a_catch_up_that_refuses().await;
    let (_, released) = hold_the_close_lock(&store, Duration::from_secs(4));

    let repository = store.repository();
    let pass = tokio::spawn(async move { crate::cache::catch_up_state(&repository).await });
    // The pass fails at once and then waits for the close lock.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let guard = publication_guard(&store.repository()).await.unwrap();
    assert!(
        released.load(Ordering::SeqCst),
        "the writer took the guard before the failed pass had closed"
    );
    drop(guard);
    let refused = pass.await.unwrap().unwrap_err();
    assert!(
        format!("{refused:#}").contains("needs one requirement"),
        "{refused:#}"
    );

    crate::cache::tests::fixtures::create_requirement(
        &store.state_store(),
        &store.scope,
        "req_next_writer",
        RequirementStatus::Active,
    );
    let stored = get_through(
        &store,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
    )
    .await
    .unwrap();
    assert_eq!(
        (stored.stamp.serial, stored.stamp.digest),
        (healthy.serial, healthy.digest)
    );
    assert_eq!(side_files(&store), Vec::<String>::new());
}

/// A read whose catch-up pass fails holds the publication guard until the
/// connection of the pass has closed, as `catch_up_state` does, and then
/// answers at the stored serial.
#[tokio::test(flavor = "multi_thread")]
async fn a_read_whose_catch_up_fails_frees_the_guard_after_its_close() {
    let (store, healthy) = a_catch_up_that_refuses().await;
    let (_, released) = hold_the_close_lock(&store, Duration::from_secs(4));

    let repository = store.repository();
    let read = tokio::spawn(async move {
        crate::operations::reader::answer(&repository, ReadPolicy::default(), |ctx| {
            Box::pin(async move {
                crate::operations::queries::records::get(ctx, super::get_query("req_overtime"))
                    .await
            })
        })
        .await
    });
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let guard = publication_guard(&store.repository()).await.unwrap();
    assert!(
        released.load(Ordering::SeqCst),
        "the writer took the guard before the failed pass had closed"
    );
    drop(guard);
    let stamped = read.await.unwrap().unwrap();
    assert_eq!(stamped.stamp.policy, StampPolicy::CatchUpFailed);
    assert_eq!(
        (stamped.stamp.serial, stamped.stamp.digest),
        (healthy.serial, healthy.digest)
    );
    assert_eq!(side_files(&store), Vec::<String>::new());
}

/// A transaction that does not begin fails the read with its own error,
/// under each policy: it is not a database that did not open.
#[tokio::test]
async fn a_read_whose_transaction_does_not_begin_keeps_the_begin_error() {
    let store = test_stores::seeded_queries();
    get_through(&store, ReadPolicy::default()).await.unwrap();
    crate::test_probes::arm("read_database_begin", || {
        Err(crate::refusal::fault!("the transaction did not begin"))
    });
    for policy in [
        FreshnessPolicy::AnnotateOnly,
        FreshnessPolicy::RefuseStale,
        FreshnessPolicy::CatchUp,
    ] {
        let failed = get_through(&store, ReadPolicy::with_freshness(policy))
            .await
            .unwrap_err();
        assert_eq!(
            format!("{failed:#}"),
            "the transaction did not begin",
            "{policy:?}"
        );
    }
    crate::test_probes::disarm("read_database_begin");
}
