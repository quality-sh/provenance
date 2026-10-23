use provenance_core::protocol::failure::{ErasedFailure as FailureEnvelope, OperationFailure};
use serde_json::Value;
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

    pub async fn run<F>(&self, operation: &str, work: F) -> Result<Value, FailureEnvelope>
    where
        F: FnOnce() -> Result<Value, FailureEnvelope> + Send + 'static,
    {
        let task = {
            let closed = self
                .state
                .closed
                .lock()
                .expect("admission lock is not poisoned");
            if *closed {
                return Err(unavailable());
            }
            let permit = self
                .state
                .admission
                .clone()
                .try_acquire_owned()
                .map_err(|_| unavailable())?;
            let task = self.state.tasks.spawn_blocking(move || {
                let _permit = permit;
                work()
            });
            // Admission and tracker registration must finish before shutdown closes it.
            drop(closed);
            task
        };
        task.await
            .map_err(|_| FailureEnvelope::new(Some(operation), OperationFailure::Internal))?
    }

    pub async fn shutdown(&self) {
        {
            let mut closed = self
                .state
                .closed
                .lock()
                .expect("admission lock is not poisoned");
            *closed = true;
            self.state.tasks.close();
            drop(closed);
        }
        self.state.tasks.wait().await;
    }
}
fn unavailable() -> FailureEnvelope {
    FailureEnvelope::new(None, OperationFailure::UnavailableNeeds)
}

#[cfg(all(test, feature = "test-fixture"))]
#[path = "execution/publication_tests.rs"]
mod publication_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use tokio::sync::oneshot;

    // Implementation aid: no queue Rule exists in the coverage scan.
    #[tokio::test]
    async fn admission_wait_has_a_bound_and_reports_host_busy() {
        let execution = Execution::new(1);
        let worker = execution.clone();
        let (started, ready) = oneshot::channel();
        let barrier = Arc::new(Barrier::new(2));
        let work_barrier = barrier.clone();
        let call = tokio::spawn(async move {
            worker
                .run("apply", move || {
                    started.send(()).unwrap();
                    work_barrier.wait();
                    Ok(Value::Null)
                })
                .await
        });
        ready.await.unwrap();
        let started_wait = tokio::time::Instant::now();
        let refused = execution
            .run("apply", || panic!("Rejected work must not run"))
            .await
            .unwrap_err();
        assert_eq!(
            refused.error,
            serde_json::json!({"kind":"host_busy"})
        );
        assert!(started_wait.elapsed() >= std::time::Duration::from_secs(2));
        tokio::task::spawn_blocking(move || barrier.wait())
            .await
            .unwrap();
        call.await.unwrap().unwrap();
        execution.shutdown().await;
    }

    // Implementation aid: no queue Rule exists in the coverage scan.
    #[tokio::test]
    async fn execution_waiters_keep_arrival_order() {
        let execution = Execution::new(1);
        let (release, wait) = std::sync::mpsc::channel();
        let (started, ready) = oneshot::channel();
        let worker = execution.clone();
        let first = tokio::spawn(async move {
            worker.run("read", move || {
                started.send(()).unwrap();
                wait.recv().unwrap();
                Ok(Value::Null)
            }).await
        });
        ready.await.unwrap();
        let order = Arc::new(Mutex::new(Vec::new()));
        let second_order = order.clone();
        let second = execution.run("read", move || {
            second_order.lock().unwrap().push(2);
            Ok(Value::Null)
        });
        tokio::pin!(second);
        assert!(futures::poll!(&mut second).is_pending());
        let third = execution.run("write", move || {
            order.lock().unwrap().push(3);
            assert_eq!(*order.lock().unwrap(), vec![2, 3]);
            Ok(Value::Null)
        });
        tokio::pin!(third);
        assert!(futures::poll!(&mut third).is_pending());
        release.send(()).unwrap();
        first.await.unwrap().unwrap();
        let (second, third) = tokio::join!(second, third);
        second.unwrap();
        third.unwrap();
        execution.shutdown().await;
    }

    // Implementation aid: shutdown must release queued calls before work finishes.
    #[tokio::test]
    async fn shutdown_ends_execution_waits_at_once() {
        let execution = Execution::new(1);
        let (release, wait) = std::sync::mpsc::channel();
        let (started, ready) = oneshot::channel();
        let worker = execution.clone();
        let first = tokio::spawn(async move {
            worker.run("read", move || {
                started.send(()).unwrap();
                wait.recv().unwrap();
                Ok(Value::Null)
            }).await
        });
        ready.await.unwrap();
        let waiting = execution.run("read", || panic!("Closed work must not run"));
        tokio::pin!(waiting);
        assert!(futures::poll!(&mut waiting).is_pending());
        let closing = execution.shutdown();
        tokio::pin!(closing);
        assert!(futures::poll!(&mut closing).is_pending());
        let refused = tokio::time::timeout(std::time::Duration::from_millis(200), waiting)
            .await.unwrap().unwrap_err();
        assert_eq!(refused.status_code(), 503);
        release.send(()).unwrap();
        first.await.unwrap().unwrap();
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
                .run("apply", move || {
                    started.send(()).unwrap();
                    work_barrier.wait();
                    finished.send(()).unwrap();
                    Ok(Value::Null)
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
        assert!(execution.run("apply", || Ok(Value::Null)).await.is_err());
    }

    #[tokio::test]
    async fn a_panicked_read_task_retains_the_internal_failure_kind() {
        let execution = Execution::default();
        let error = execution
            .run("get", || panic!("injected read task failure"))
            .await
            .unwrap_err();
        execution.shutdown().await;
        assert_eq!(error.error, serde_json::json!({"kind":"internal"}));
        assert_eq!(
            serde_json::to_value(error.meta).unwrap(),
            serde_json::json!({})
        );
    }
}
