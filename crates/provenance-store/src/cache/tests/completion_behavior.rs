//! Connection completion belongs to the cache. A connection opened for
//! work that fails early, a connection that is dropped without a waited
//! close, and a connection behind a refused read all end with the ordered
//! close, so the read or the report never leaves the teardown unordered.

use super::super::*;
use super::fixtures;
use super::wal_behavior::wal_files;
use crate::layout::ProvenanceLayout;
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::operations::reader::ReadRefusal;
use std::path::PathBuf;
use std::time::Duration;

/// How long a detached completion may take once nothing blocks it.
const COMPLETION_LIMIT: Duration = Duration::from_secs(10);

/// Where the completion of this layout's database holds its close lock.
fn close_lock_path(layout: &ProvenanceLayout) -> PathBuf {
    layout
        .cache_db_path()
        .with_extension("db.close.lock")
        .into_std_path_buf()
}

/// A finished completion holds the close lock until the physical
/// teardown has finished, so the lock file exists and no `-wal` or `-shm`
/// file remains.
fn assert_completed(layout: &ProvenanceLayout) {
    assert!(
        close_lock_path(layout).exists(),
        "the completion must run the ordered close"
    );
    assert!(
        wal_files(layout).is_empty(),
        "the completion must leave no `-wal` or `-shm` files"
    );
}

/// Waits for a detached completion, which has no result the test can
/// await, until the close lock exists and the files are gone.
async fn wait_for_completion(layout: &ProvenanceLayout) {
    tokio::time::timeout(COMPLETION_LIMIT, async {
        loop {
            if close_lock_path(layout).exists() && wal_files(layout).is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the dropped connection must complete its close");
}

/// A first materialization that fails after its pool is open is an early
/// failure. The report carries the failure, and the connection has
/// already completed.
#[tokio::test]
async fn a_failed_first_materialization_completes_its_connection() {
    let (_dir, layout, _scope) = fixtures::empty_layout();
    crate::test_probes::arm("run_migrations_under_guard", || {
        anyhow::bail!("injected migration failure")
    });
    let result = materialize_empty_state(&layout).await;
    crate::test_probes::disarm("run_migrations_under_guard");
    let error = result.expect_err("the injected failure must surface");
    assert!(
        error.to_string().contains("injected migration failure"),
        "{error:#}"
    );
    assert_completed(&layout);
}

/// The rebuild body opens its own pool beside the caller's. A failure
/// after that open must complete the body's connection, not drop it raw.
#[tokio::test]
async fn a_failed_rebuild_materialization_completes_its_connection() {
    let (_dir, layout, _scope) = fixtures::empty_layout();
    crate::test_probes::arm("run_migrations_under_guard", || {
        anyhow::bail!("injected migration failure")
    });
    let result = materialize_state(&layout).await;
    crate::test_probes::disarm("run_migrations_under_guard");
    let error = result.expect_err("the injected failure must surface");
    assert!(
        error.to_string().contains("injected migration failure"),
        "{error:#}"
    );
    assert_completed(&layout);
}

/// A read that refuses stale canonical state must still complete its
/// connection before the refusal leaves the reader.
#[tokio::test]
async fn a_refused_stale_read_completes_its_connection() {
    let (_dir, layout, scope) = fixtures::seeded_layout();
    catch_up_state(&layout).await.unwrap();
    let path = crate::shards::requirements_path(&layout, &scope);
    std::fs::write(&path, b"not JSON\n").unwrap();
    let error = crate::operations::reader::answer(
        layout.root(),
        &scope,
        ReadPolicy::with_freshness(FreshnessPolicy::RefuseStale),
        |_| Box::pin(async { Ok(1usize) }),
    )
    .await
    .expect_err("the moved unit must refuse the read");
    assert!(
        matches!(
            error.downcast_ref::<ReadRefusal>(),
            Some(ReadRefusal::Stale { .. })
        ),
        "{error:#}"
    );
    assert_completed(&layout);
}

/// A read that refuses a projection behind on migrations must still
/// complete its connection before the refusal leaves the reader.
#[tokio::test]
async fn a_refused_schema_behind_read_completes_its_connection() {
    let (_dir, layout, scope) = fixtures::seeded_layout();
    catch_up_state(&layout).await.unwrap();
    let cache = open_existing_cache(&layout).await.unwrap();
    sqlx::query("DELETE FROM _schema_migrations")
        .execute(cache.pool())
        .await
        .unwrap();
    cache.close().await.unwrap();
    let error = crate::operations::reader::answer(
        layout.root(),
        &scope,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
        |_| Box::pin(async { Ok(1usize) }),
    )
    .await
    .expect_err("a projection behind on migrations must refuse");
    assert!(
        matches!(
            error.downcast_ref::<ReadRefusal>(),
            Some(ReadRefusal::SchemaBehind { .. })
        ),
        "{error:#}"
    );
    assert_completed(&layout);
}

/// A catch-up step that fails while the database holds a revision answers
/// at the stored serial, and the failed step's connection is complete
/// before the answer leaves the reader.
#[tokio::test]
async fn a_failed_catch_up_step_completes_its_connection() {
    let (_dir, layout, scope) = fixtures::seeded_layout();
    let healthy =
        crate::operations::reader::answer(layout.root(), &scope, ReadPolicy::default(), |_| {
            Box::pin(async { Ok(1usize) })
        })
        .await
        .unwrap();
    crate::test_probes::arm("run_migrations_under_guard", || {
        anyhow::bail!("injected catch-up failure")
    });
    let stamped =
        crate::operations::reader::answer(layout.root(), &scope, ReadPolicy::default(), |_| {
            Box::pin(async { Ok(1usize) })
        })
        .await
        .unwrap();
    crate::test_probes::disarm("run_migrations_under_guard");
    assert_eq!(
        stamped.stamp.policy,
        provenance_core::protocol::StampPolicy::CatchUpFailed
    );
    assert_eq!(stamped.stamp.serial, healthy.stamp.serial);
    assert!(stamped
        .freshness_error
        .unwrap()
        .contains("injected catch-up failure"));
    assert_completed(&layout);
}

/// A connection dropped without a waited close starts the same ordered
/// completion on its own task, so a forgotten close cannot leak an
/// unordered pool.
#[tokio::test]
async fn a_dropped_connection_completes_without_a_waited_close() {
    let (_dir, layout, _scope) = fixtures::empty_layout();
    let connection = open_cache(&layout).await.unwrap();
    sqlx::query("CREATE TABLE probe (x INTEGER)")
        .execute(connection.pool())
        .await
        .unwrap();
    drop(connection);
    wait_for_completion(&layout).await;
}
