use provenance_core::protocol::failure::{ErasedFailure, OperationFailure};
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;
use tokio_util::task::TaskTracker;

#[derive(Clone)]
pub struct Execution {
    state: Arc<State>,
}
struct State {
    closed: Mutex<bool>,
    admission: Arc<Semaphore>,
    tasks: TaskTracker,
}
impl Default for Execution {
    fn default() -> Self {
        Self::new(8)
    }
}
impl Execution {
    fn new(capacity: usize) -> Self {
        Self {
            state: Arc::new(State {
                closed: Mutex::new(false),
                admission: Arc::new(Semaphore::new(capacity)),
                tasks: TaskTracker::new(),
            }),
        }
    }

    /// Waits for a work slot, then runs the work on the tracked blocking pool.
    pub async fn run<T, F>(&self, work: F) -> Result<T, OperationFailure>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let permit = crate::admission::acquire(self.state.admission.clone())
            .await
            .map_err(refusal)?;
        let task = {
            let closed = self
                .state
                .closed
                .lock()
                .expect("admission lock is not poisoned");
            if *closed {
                return Err(OperationFailure::UnavailableNeeds);
            }
            let task = self.state.tasks.spawn_blocking(move || {
                let _permit = permit;
                work()
            });
            // Admission and tracker registration must finish before shutdown closes it.
            drop(closed);
            task
        };
        task.await.map_err(|_| OperationFailure::Internal)
    }

    pub async fn shutdown(&self) {
        {
            let mut closed = self
                .state
                .closed
                .lock()
                .expect("admission lock is not poisoned");
            *closed = true;
            self.state.admission.close();
            self.state.tasks.close();
            drop(closed);
        }
        self.state.tasks.wait().await;
    }
}

/// The admission wait answers with an erased refusal; the typed caller gets the common failure back.
fn refusal(refused: ErasedFailure) -> OperationFailure {
    serde_json::from_value(refused.error).unwrap_or(OperationFailure::Internal)
}

#[cfg(all(test, feature = "test-fixture"))]
#[path = "execution/publication_tests.rs"]
mod publication_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use provenance_macros::verifies;
    use std::sync::{Arc, Barrier};
    use tokio::sync::oneshot;

    struct HeldWork {
        release: std::sync::mpsc::Sender<()>,
        call: tokio::task::JoinHandle<Result<(), OperationFailure>>,
    }

    async fn hold_work(execution: &Execution) -> HeldWork {
        let worker = execution.clone();
        let (started, ready) = oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let call = tokio::spawn(async move {
            worker
                .run(move || {
                    let _ = started.send(());
                    let _ = wait.recv();
                })
                .await
        });
        ready.await.unwrap();
        HeldWork { release, call }
    }

    #[verifies("rule_host_slot_refusal_kind", examples)]
    #[tokio::test(start_paused = true)]
    async fn admission_wait_has_a_bound_and_reports_host_busy() {
        let execution = Execution::new(1);
        let HeldWork { release, call } = hold_work(&execution).await;
        let started_wait = tokio::time::Instant::now();
        let waiting = execution.run::<(), _>(|| panic!("Rejected work must not run"));
        tokio::pin!(waiting);
        assert!(futures::poll!(&mut waiting).is_pending());
        tokio::time::advance(std::time::Duration::from_millis(2999)).await;
        assert!(futures::poll!(&mut waiting).is_pending());
        tokio::time::advance(std::time::Duration::from_millis(1)).await;
        let refused = waiting.await.unwrap_err();
        assert_eq!(started_wait.elapsed(), std::time::Duration::from_secs(3));
        release.send(()).unwrap();
        call.await.unwrap().unwrap();
        assert_eq!(refused, OperationFailure::HostBusy);
        execution.shutdown().await;
    }

    #[verifies("rule_host_slot_queue_bound", examples)]
    #[tokio::test]
    async fn execution_waiters_keep_arrival_order() {
        let execution = Execution::new(1);
        let HeldWork { release, call } = hold_work(&execution).await;
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut waiters = Vec::new();
        for index in 0..5 {
            let order = order.clone();
            let mut waiter = Box::pin(execution.run(move || {
                order.lock().unwrap().push(index);
            }));
            assert!(futures::poll!(&mut waiter).is_pending());
            waiters.push(waiter);
        }
        release.send(()).unwrap();
        call.await.unwrap().unwrap();
        // Poll in reverse order so task polling cannot supply the expected order.
        for result in futures::future::join_all(waiters.into_iter().rev()).await {
            result.unwrap();
        }
        assert_eq!(*order.lock().unwrap(), vec![0, 1, 2, 3, 4]);
        execution.shutdown().await;
    }

    #[verifies("rule_host_slot_refusal_kind", examples)]
    #[tokio::test]
    async fn shutdown_ends_execution_waits_at_once() {
        let execution = Execution::new(1);
        let HeldWork { release, call } = hold_work(&execution).await;
        let waiting = execution.run::<(), _>(|| panic!("Closed work must not run"));
        tokio::pin!(waiting);
        assert!(futures::poll!(&mut waiting).is_pending());
        let closing = execution.shutdown();
        tokio::pin!(closing);
        assert!(futures::poll!(&mut closing).is_pending());
        let refused = tokio::time::timeout(std::time::Duration::from_millis(200), waiting)
            .await
            .unwrap()
            .unwrap_err();
        assert_eq!(refused.status_code(), 503);
        assert_eq!(refused, OperationFailure::UnavailableNeeds);
        release.send(()).unwrap();
        call.await.unwrap().unwrap();
        closing.await;
    }

    #[tokio::test]
    async fn cancellation_keeps_work_alive_and_shutdown_joins_it() {
        let execution = Execution::default();
        let barrier = Arc::new(Barrier::new(2));
        let (started, ready) = oneshot::channel();
        let (finished, done) = oneshot::channel();
        let work_barrier = barrier.clone();
        let worker = execution.clone();
        let caller = tokio::spawn(async move {
            worker
                .run(move || {
                    started.send(()).unwrap();
                    work_barrier.wait();
                    finished.send(()).unwrap();
                })
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), ready)
            .await
            .expect("blocking work must start")
            .unwrap();
        caller.abort();
        let closer = execution.clone();
        let closing = tokio::spawn(async move {
            closer.shutdown().await;
        });
        tokio::task::yield_now().await;
        assert!(!closing.is_finished());
        tokio::task::spawn_blocking(move || barrier.wait())
            .await
            .unwrap();
        closing.await.unwrap();
        done.await.unwrap();
        assert!(execution.run(|| ()).await.is_err());
    }

    #[tokio::test]
    async fn a_panicked_read_task_retains_the_internal_failure_kind() {
        let execution = Execution::default();
        let error = execution
            .run::<(), _>(|| panic!("injected read task failure"))
            .await
            .unwrap_err();
        execution.shutdown().await;
        assert_eq!(error, OperationFailure::Internal);
    }
}
