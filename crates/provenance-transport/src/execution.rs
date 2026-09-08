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

    pub async fn run<F>(&self, work: F) -> Result<Value, FailureEnvelope>
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
            .map_err(|_| FailureEnvelope::new(None, OperationFailure::Internal))?
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn admission_refuses_excess_work_without_running_it() {
        let execution = Execution::new(1);
        let worker = execution.clone();
        let (started, ready) = oneshot::channel();
        let barrier = Arc::new(Barrier::new(2));
        let work_barrier = barrier.clone();
        let call = tokio::spawn(async move {
            worker
                .run(move || {
                    started.send(()).unwrap();
                    work_barrier.wait();
                    Ok(Value::Null)
                })
                .await
        });
        ready.await.unwrap();
        let refused = execution
            .run(|| panic!("Rejected work must not run"))
            .await
            .unwrap_err();
        assert_eq!(
            refused.error,
            serde_json::json!({"kind":"unavailable_needs"})
        );
        tokio::task::spawn_blocking(move || barrier.wait())
            .await
            .unwrap();
        call.await.unwrap().unwrap();
        execution.shutdown().await;
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
        assert!(execution.run(|| Ok(Value::Null)).await.is_err());
    }
}
