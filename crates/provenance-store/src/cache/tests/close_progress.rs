//! Close must finish when publication waiters fill the blocking pool.
use crate::layout::ProvenanceLayout;
use anyhow::Context;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[test]
fn failed_freshness_closes_with_one_busy_blocking_worker() {
    failed_freshness_close(Duration::ZERO);
}

#[test]
fn failed_freshness_close_allows_slow_setup() {
    failed_freshness_close(Duration::from_millis(500));
}

fn failed_freshness_close(setup_delay: Duration) {
    const SETUP_LIMIT: Duration = Duration::from_secs(5);
    const CLOSE_LIMIT: Duration = Duration::from_millis(300);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let result: anyhow::Result<()> = runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(root.clone());
        let scope = provenance_core::ScopeId::new("default").unwrap();
        let guard = tokio::time::timeout(
            SETUP_LIMIT,
            crate::publication::publication_guard(&layout),
        )
        .await
        .context("the setup publication guard must open")??;
        let release_setup = tokio::spawn(async move {
            tokio::time::sleep(setup_delay).await;
            drop(guard);
        });
        let waiter = Arc::new(Mutex::new(None));
        let waiter_out = Arc::clone(&waiter);
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        let mut signals = Some((started_tx, finished_tx));
        crate::test_probes::arm("run_migrations_under_guard", move || {
            let (started_tx, finished_tx) = signals.take().context("the probe must run once")?;
            let competing_layout = layout.clone();
            let (entered_tx, entered_rx) = std::sync::mpsc::channel();
            let handle = tokio::task::spawn_blocking(move || {
                entered_tx.send(())?;
                let result =
                    crate::publication::with_repository_publication(&competing_layout, || Ok(()));
                let _ = finished_tx.send(());
                result
            });
            *waiter_out.lock().unwrap() = Some(handle);
            entered_rx.recv_timeout(SETUP_LIMIT)?;
            // The read holds the publication lock and the only blocking
            // worker has entered its competing publication call.
            let _ = started_tx.send(tokio::time::Instant::now());
            anyhow::bail!("verification: the freshness step fails while another publication waits")
        });
        let result = tokio::time::timeout(SETUP_LIMIT, async {
            let read = async {
                let answer = crate::operations::reader::answer(
                    &root,
                    &scope,
                    crate::operations::read_policy::ReadPolicy::default(),
                    |_| Box::pin(async { Ok(1usize) }),
                )
                .await;
                anyhow::ensure!(
                    answer.is_err(),
                    "the freshness failure must surface: {answer:?}"
                );
                Ok::<(), anyhow::Error>(())
            };
            let progress = async {
                let started = started_rx.await.context("the contention probe must run")?;
                // The waiter can finish only after the read closes its cache
                // and releases its publication guard. Cache setup and the
                // fallback read do not use this close budget.
                tokio::time::timeout_at(started + CLOSE_LIMIT, finished_rx)
                    .await
                    .context("close must finish with a busy blocking pool")?
                    .context("the publication waiter must report completion")?;
                Ok::<(), anyhow::Error>(())
            };
            tokio::try_join!(read, progress)?;
            Ok::<(), anyhow::Error>(())
        })
        .await
        .context("the freshness read must finish within the setup limit")
        .and_then(std::convert::identity);
        // Both futures have been dropped, including the publication guard
        // on failure. Clean up before reporting an assertion failure.
        crate::test_probes::disarm("run_migrations_under_guard");
        release_setup.await?;
        let handle = waiter.lock().unwrap().take();
        let entered = handle.is_some();
        if let Some(handle) = handle {
            tokio::time::timeout(SETUP_LIMIT, handle)
                .await
                .context("the publication waiter must finish after cancellation")???;
        }
        result?;
        anyhow::ensure!(
            entered,
            "the contention probe must start a publication waiter"
        );
        Ok(())
    });
    runtime.shutdown_timeout(Duration::from_secs(2));
    result.unwrap();
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
