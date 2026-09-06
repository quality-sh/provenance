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
#[ignore = "concurrent pool closes can leave a -wal file; reader changes are outside K.4"]
#[verifies("rule_completed_read_leaves_no_wal_files", examples)]
async fn answers_that_close_together_remove_the_wal_files() {
    assert_concurrent_answers(true).await;
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
