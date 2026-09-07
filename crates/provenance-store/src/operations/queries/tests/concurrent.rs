use super::{root_of, seeded_store};
use crate::operations::read_policy::ReadPolicy;
use crate::operations::reader::answer;
use provenance_core::protocol::StampPolicy;
use provenance_core::Requirement;
use provenance_macros::verifies;
use std::{sync::Arc, time::Duration};
use tokio::sync::{Barrier, Notify};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[verifies("rule_completed_read_leaves_no_wal_files", examples)]
async fn concurrent_answers_finish_and_remove_the_wal_files() {
    assert_concurrent_answers(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[verifies("rule_completed_read_leaves_no_wal_files", examples)]
async fn answers_that_close_together_remove_the_wal_files() {
    assert_concurrent_answers(true).await;
}

/// Eight answers whose snapshots all overlap and whose closes then start
/// together must also leave no -wal or -shm file behind: the close order
/// holds for more than two readers at once.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[verifies("rule_completed_read_leaves_no_wal_files", examples)]
async fn eight_answers_that_close_together_remove_the_wal_files() {
    const READERS: usize = 8;
    let (dir, store, scope) = seeded_store();
    let root = root_of(&dir);
    let barrier = Arc::new(Barrier::new(READERS));
    let readers = (0..READERS)
        .map(|_| {
            let root = root.clone();
            let scope = scope.clone();
            let barrier = Arc::clone(&barrier);
            tokio::spawn(async move {
                answer(&root, &scope, ReadPolicy::default(), move |ctx| {
                    Box::pin(async move {
                        let count = ctx.snapshot().table::<Requirement>().count().await?;
                        // All eight snapshots must be open before any answer
                        // can finish.
                        barrier.wait().await;
                        Ok(count)
                    })
                })
                .await
            })
        })
        .collect::<Vec<_>>();
    let answers = tokio::time::timeout(Duration::from_secs(15), async {
        let mut answers = Vec::new();
        for reader in readers {
            answers.push(reader.await.expect("each read task must join"));
        }
        answers
    })
    .await
    .expect("all eight reads must answer while their snapshots overlap");
    let mut serials = Vec::new();
    for answer in &answers {
        let answer = answer.as_ref().expect("each read must answer");
        assert_eq!(answer.result, 1);
        assert_eq!(answer.stamp.policy, StampPolicy::CatchUp);
        assert!(
            answer.freshness_error.is_none(),
            "{:?}",
            answer.freshness_error
        );
        serials.push(answer.stamp.serial);
    }
    let serial = answers[0]
        .as_ref()
        .expect("the first read must answer")
        .stamp
        .serial;
    assert!(
        serials.iter().all(|seen| seen == &serial),
        "all eight reads must answer at the same serial: {serials:?}"
    );
    assert_eq!(
        answers[0]
            .as_ref()
            .expect("the first read must answer")
            .stamp
            .instance_id,
        answers[READERS - 1]
            .as_ref()
            .expect("the last read must answer")
            .stamp
            .instance_id
    );
    for suffix in ["-wal", "-shm"] {
        let path = format!("{}{suffix}", store.layout.cache_db_path());
        assert!(!std::path::Path::new(&path).exists(), "read left {path}");
    }
}

async fn assert_concurrent_answers(close_together: bool) {
    let (dir, store, scope) = seeded_store();
    let root = root_of(&dir);
    let barrier = Arc::new(Barrier::new(2));
    let first_closed = Arc::new(Notify::new());
    let start = |wait_for_first| {
        let root = root.clone();
        let scope = scope.clone();
        let barrier = Arc::clone(&barrier);
        let first_closed = Arc::clone(&first_closed);
        tokio::spawn(async move {
            answer(&root, &scope, ReadPolicy::default(), move |ctx| {
                Box::pin(async move {
                    let count = ctx.snapshot().table::<Requirement>().count().await?;
                    // Both snapshots must be open before either answer can finish.
                    barrier.wait().await;
                    if wait_for_first {
                        first_closed.notified().await;
                    }
                    Ok(count)
                })
            })
            .await
        })
    };
    let first = start(false);
    let second = start(!close_together);
    let (first, second) = tokio::time::timeout(Duration::from_secs(15), async {
        let finish_first = async {
            let result = first.await;
            first_closed.notify_one();
            result
        };
        tokio::join!(finish_first, second)
    })
    .await
    .expect("both reads must answer while their snapshots overlap");
    let first = first.unwrap().unwrap();
    let second = second.unwrap().unwrap();
    assert_eq!(first.result, 1);
    assert_eq!(second.result, 1);
    assert_eq!(first.stamp.policy, StampPolicy::CatchUp);
    assert_eq!(second.stamp.policy, StampPolicy::CatchUp);
    assert!(
        first.freshness_error.is_none(),
        "{:?}",
        first.freshness_error
    );
    assert!(
        second.freshness_error.is_none(),
        "{:?}",
        second.freshness_error
    );
    assert_eq!(first.stamp.serial, second.stamp.serial);
    assert_eq!(first.stamp.instance_id, second.stamp.instance_id);
    for suffix in ["-wal", "-shm"] {
        let path = format!("{}{suffix}", store.layout.cache_db_path());
        assert!(!std::path::Path::new(&path).exists(), "read left {path}");
    }
}
