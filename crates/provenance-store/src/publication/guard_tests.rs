use super::*;
use std::time::Duration;

fn test_layout() -> (tempfile::TempDir, ProvenanceLayout) {
    let directory = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    (directory, ProvenanceLayout::new(root))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guard_serializes_against_the_synchronous_publication_entry() {
    let (_dir, layout) = test_layout();
    let guard = guard::publication_guard(&layout).await.unwrap();
    let sync_layout = layout.clone();
    let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let signal = finished.clone();
    let worker = std::thread::spawn(move || {
        with_repository_publication(&sync_layout, || {
            signal.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .unwrap();
    });
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        !finished.load(std::sync::atomic::Ordering::SeqCst),
        "a synchronous publication must wait while the guard is held"
    );
    drop(guard);
    worker.join().unwrap();
    assert!(finished.load(std::sync::atomic::Ordering::SeqCst));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_guard_waits_until_the_first_is_dropped() {
    let (_dir, layout) = test_layout();
    let first = guard::publication_guard(&layout).await.unwrap();
    let second = tokio::time::timeout(
        Duration::from_millis(100),
        guard::publication_guard(&layout),
    )
    .await;
    assert!(
        second.is_err(),
        "second guard must wait while the first is held"
    );
    drop(first);
    let second = tokio::time::timeout(
        Duration::from_millis(1000),
        guard::publication_guard(&layout),
    )
    .await
    .expect("second guard must acquire after the first drops");
    drop(second);
}

#[tokio::test]
async fn snapshot_state_under_guard_runs_inside_a_held_guard() {
    let (_dir, layout) = test_layout();
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(layout.manifest_path(), "{}").unwrap();
    let guard = guard::publication_guard(&layout).await.unwrap();

    let snapshot = guard::snapshot_state_under_guard(&guard, &layout).unwrap();

    assert!(snapshot.layout().manifest_path().exists());
}

#[test]
fn guard_entry_honors_the_read_only_bypass() {
    let (_dir, layout) = test_layout();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let read_only_guard = with_read_only_validation(&layout, || {
        let inner = runtime.block_on(guard::publication_guard(&layout)).unwrap();
        assert!(
            !inner.holds_file_lock(),
            "a read-only validation must not take the publication file lock"
        );
        Ok(())
    });
    drop(read_only_guard);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_held_scope_opens_a_second_lock_file_description() {
    let (_dir, layout) = test_layout();
    let guard = guard::publication_guard(&layout).await.unwrap();
    let lock_path = layout.publication_lock_path();
    // While the guard holds the lock, a plain blocking acquire of the same
    // file must not succeed: one held scope, one file description.
    let probe = std::thread::spawn(move || crate::jsonl::with_advisory_lock(&lock_path, || Ok(())));
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        !probe.is_finished(),
        "probe must block while the guard holds the lock"
    );
    drop(guard);
    probe.join().unwrap().unwrap();
}
