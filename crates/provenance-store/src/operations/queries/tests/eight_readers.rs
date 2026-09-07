use super::{root_of, seeded_store};
use crate::operations::read_policy::ReadPolicy;
use crate::operations::reader::answer;
use provenance_core::Requirement;
use std::{sync::Arc, time::Duration};
use tokio::sync::Barrier;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn many_readers_close_together() {
    let (dir, store, scope) = seeded_store();
    let n = 8;
    let barrier = Arc::new(Barrier::new(n));
    let mut tasks = Vec::new();
    for _ in 0..n {
        let root = root_of(&dir);
        let scope = scope.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            answer(&root, &scope, ReadPolicy::default(), move |ctx| {
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
