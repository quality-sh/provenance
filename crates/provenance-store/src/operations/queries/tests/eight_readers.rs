use super::{repository_at, root_of, seeded_store};
use crate::operations::read_policy::ReadPolicy;
use crate::operations::reader::answer;
use provenance_core::Requirement;
use std::{sync::Arc, time::Duration};
use tokio::sync::Barrier;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn many_readers_close_together() {
    let (dir, store, _scope) = seeded_store();
    // Open once: an open inside a task would wait for the lock on a worker.
    let repository = repository_at(&root_of(&dir));
    let n = 8;
    let barrier = Arc::new(Barrier::new(n));
    let mut tasks = Vec::new();
    for _ in 0..n {
        let repository = repository.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            answer(&repository, ReadPolicy::default(), move |ctx| {
                Box::pin(async move {
                    let count = ctx.snapshot().table::<Requirement>().count().await?;
                    barrier.wait().await;
                    Ok(count)
                })
            })
            .await
        }));
    }
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut serial = None;
        for task in tasks {
            let got = task.await.unwrap().unwrap();
            assert_eq!(got.result, 1);
            assert!(got.freshness_error.is_none());
            if let Some(serial) = serial {
                assert_eq!(got.stamp.serial, serial);
            }
            serial = Some(got.stamp.serial);
        }
    })
    .await
    .unwrap();
    for suffix in ["-wal", "-shm"] {
        let path = format!("{}{suffix}", store.layout.cache_db_path());
        assert!(!std::path::Path::new(&path).exists(), "read left {path}");
    }
}
