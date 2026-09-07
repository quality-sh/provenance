//! Close must finish when publication waiters fill the blocking pool.
use crate::layout::ProvenanceLayout;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[test]
fn failed_freshness_closes_with_one_busy_blocking_worker() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(root.clone());
        let scope = provenance_core::ScopeId::new("default").unwrap();
        let waiter = Arc::new(Mutex::new(None));
        let waiter_out = Arc::clone(&waiter);
        crate::test_probes::arm("run_migrations_under_guard", move || {
            let competing_layout = layout.clone();
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            let handle = tokio::task::spawn_blocking(move || {
                entered_tx.send(()).unwrap();
                crate::publication::with_repository_publication(&competing_layout, || Ok(()))
            });
            // The only blocking worker is now running a competing public
            // publication call. This read still owns the publication lock.
            entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            *waiter_out.lock().unwrap() = Some(handle);
            anyhow::bail!("verification: the freshness step fails while another publication waits")
        });
        let started = Instant::now();
        let result = tokio::time::timeout(
            Duration::from_millis(300),
            crate::operations::reader::answer(
                &root,
                &scope,
                crate::operations::read_policy::ReadPolicy::default(),
                |_| Box::pin(async { Ok(1usize) }),
            ),
        ).await;
        let elapsed = started.elapsed();
        crate::test_probes::disarm("run_migrations_under_guard");
        println!("blocking_workers=1 read_timed_out={} cycle=read_holds_publication_lock/worker_waits_publication_lock/read_close_waits_worker", result.is_err());
        let timed_out = result.is_err();
        if let Ok(result) = result {
            println!("read_finished_without_timeout result={result:?}");
        }
        let handle = waiter.lock().unwrap().take().unwrap();
        tokio::time::timeout(Duration::from_secs(5), handle).await.unwrap().unwrap().unwrap();
        println!("blocking_workers=1 elapsed_ms={} timed_out={timed_out}", elapsed.as_millis());
        assert!(!timed_out, "close must finish with a busy blocking pool");
    });
    runtime.shutdown_timeout(Duration::from_secs(2));
}

#[test]
fn materialization_closes_with_four_busy_blocking_workers() {
    const WORKERS: usize = 4;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(WORKERS)
        .build()
        .unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(root);
        let probe_layout = layout.clone();
        let waiters = Arc::new(Mutex::new(Vec::new()));
        let waiters_out = Arc::clone(&waiters);
        crate::test_probes::arm("run_migrations_under_guard", move || {
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            for _ in 0..WORKERS {
                let competing_layout = probe_layout.clone();
                let entered_tx = entered_tx.clone();
                let handle = tokio::task::spawn_blocking(move || {
                    entered_tx.send(()).unwrap();
                    crate::publication::with_repository_publication(&competing_layout, || Ok(()))
                });
                waiters_out.lock().unwrap().push(handle);
            }
            for _ in 0..WORKERS {
                entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            // This probe observes scheduling only; the operation continues.
            Ok(())
        });
        let started = Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            crate::cache::materialize_empty_state(&layout),
        )
        .await;
        let elapsed = started.elapsed();
        crate::test_probes::disarm("run_migrations_under_guard");
        let timed_out = result.is_err();
        println!(
            "blocking_workers={WORKERS} materialization_timed_out={timed_out} fault_injected=false"
        );
        if let Ok(result) = result {
            println!("materialization_finished result={result:?}");
            assert!(result.is_ok(), "the materialization itself must be valid");
        }
        let handles = std::mem::take(&mut *waiters.lock().unwrap());
        for handle in handles {
            tokio::time::timeout(Duration::from_secs(5), handle)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        }
        println!(
            "blocking_workers={WORKERS} elapsed_ms={} timed_out={timed_out}",
            elapsed.as_millis()
        );
        assert!(!timed_out, "close must finish with a busy blocking pool");
    });
    runtime.shutdown_timeout(Duration::from_secs(2));
}
