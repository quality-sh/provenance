//! The publication guard covers the freshness step only. The answer is
//! read outside it from a snapshot pinned in one read transaction.

use super::super::comparison::test_stores;
use super::get_query;
use crate::cache::tests::fixtures::create_requirement;
use crate::cache::{catch_up_state, catch_up_with_guard, open_cache, ProjectionFamily};
use crate::operations::queries::records;
use crate::operations::read_policy::ReadPolicy;
use crate::operations::reader::{answer, ReadSnapshot};
use crate::publication::{publication_guard, with_repository_publication};
use crate::test_probes::publication_lock_is_held;
use provenance_core::RequirementStatus;
use std::sync::mpsc;
use std::time::Duration;

#[tokio::test]
async fn the_publication_lock_is_free_while_a_read_answers() {
    let store = test_stores::seeded_queries();
    let layout = store.layout();
    let held_during_catch_up = std::rc::Rc::new(std::cell::Cell::new(false));
    let probe_layout = layout.clone();
    let probe_seen = held_during_catch_up.clone();
    crate::test_probes::arm("run_migrations_under_guard", move || {
        probe_seen.set(publication_lock_is_held(&probe_layout));
        Ok(())
    });
    let scope = store.scope.clone();
    let run_layout = layout.clone();
    let stamped = answer(
        &store.root,
        &store.scope,
        ReadPolicy::default(),
        move |ctx| {
            Box::pin(async move {
                anyhow::ensure!(
                    !publication_lock_is_held(&run_layout),
                    "the lock must be free while the read answers"
                );
                let found = records::get(ctx, &scope, get_query("req_overtime"))?;
                anyhow::ensure!(
                    !publication_lock_is_held(&run_layout),
                    "the lock must be free after the operation read canonical state"
                );
                Ok(found)
            })
        },
    )
    .await
    .unwrap();
    crate::test_probes::disarm("run_migrations_under_guard");
    assert!(stamped.result.found);
    assert!(
        held_during_catch_up.get(),
        "catch-up itself must run under the lock"
    );
}

#[tokio::test]
async fn a_canonical_write_does_not_wait_for_a_read() {
    let store = test_stores::seeded_queries();
    let layout = store.layout();
    let scope = store.scope.clone();
    let stamped = answer(
        &store.root,
        &store.scope,
        ReadPolicy::default(),
        move |ctx| {
            Box::pin(async move {
                let (sender, receiver) = mpsc::channel();
                let writer_layout = layout.clone();
                let writer = std::thread::spawn(move || {
                    with_repository_publication(&writer_layout, || {
                        sender.send(()).unwrap();
                        Ok(())
                    })
                });
                receiver
                    .recv_timeout(Duration::from_secs(5))
                    .expect("a canonical write must not wait on an open read");
                writer.join().unwrap()?;
                records::get(ctx, &scope, get_query("req_overtime"))
            })
        },
    )
    .await
    .unwrap();
    assert!(stamped.result.found);
}

/// The first snapshot keeps its pool's one connection for its whole life,
/// so the publication and the second snapshot run on a second pool; WAL
/// is what lets both read the same file.
#[tokio::test]
async fn a_read_that_started_before_a_publication_answers_at_its_serial() {
    let store = test_stores::seeded_queries();
    let layout = store.layout();
    catch_up_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let second_pool = open_cache(&layout).await.unwrap();

    let first = ReadSnapshot::open(&pool, &store.scope)
        .await
        .unwrap()
        .expect("a revision");
    let before = first
        .table(ProjectionFamily::Requirements)
        .count()
        .await
        .unwrap();
    assert_eq!(before, 1);

    create_requirement(
        &store.state_store(),
        &store.scope,
        "req_second",
        RequirementStatus::Active,
    );
    let guard = publication_guard(&layout).await.unwrap();
    let report = catch_up_with_guard(&guard, &second_pool, &layout)
        .await
        .unwrap();
    drop(guard);
    assert_eq!(report.serial, first.serial() + 1);

    assert_eq!(
        first
            .table(ProjectionFamily::Requirements)
            .count()
            .await
            .unwrap(),
        before,
        "the open snapshot still reads its own serial"
    );
    let second = ReadSnapshot::open(&second_pool, &store.scope)
        .await
        .unwrap()
        .expect("a revision");
    assert_eq!(second.serial(), first.serial() + 1);
    assert_eq!(
        second
            .table(ProjectionFamily::Requirements)
            .count()
            .await
            .unwrap(),
        before + 1
    );
    drop(first);
    drop(second);
    pool.close().await;
    second_pool.close().await;
}
