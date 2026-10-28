//! Shared bounded admission for ingress and operation work.

use provenance_core::protocol::failure::{ErasedFailure, OperationFailure};
use provenance_macros::rule;
use std::{future::Future, sync::Arc, time::Duration};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    time::Instant,
};

// Three seconds absorbs short request bursts without an unbounded slot wait.
const WAIT: Duration = Duration::from_secs(3);

tokio::task_local! {
    static REQUEST_DEADLINE: Instant;
}

pub async fn with_deadline<F: Future>(work: F) -> F::Output {
    REQUEST_DEADLINE.scope(Instant::now() + WAIT, work).await
}

/// The deadline of the current request, or a fresh one for work outside a request.
pub fn deadline() -> Instant {
    REQUEST_DEADLINE
        .try_with(|deadline| *deadline)
        .unwrap_or_else(|_| Instant::now() + WAIT)
}

/// Wait for a slot in arrival order until the request deadline.
#[rule("rule_host_slot_queue_bound")]
#[rule("rule_host_slot_refusal_kind")]
pub async fn acquire(semaphore: Arc<Semaphore>) -> Result<OwnedSemaphorePermit, ErasedFailure> {
    let deadline = deadline();
    // A closed host is unavailable, even when a request has time left to wait.
    tokio::time::timeout_at(deadline, semaphore.acquire_owned())
        .await
        .map_err(|_| busy())?
        .map_err(|_| unavailable())
}

pub fn busy() -> ErasedFailure {
    ErasedFailure::new(None, OperationFailure::HostBusy)
}

pub fn unavailable() -> ErasedFailure {
    ErasedFailure::new(None, OperationFailure::UnavailableNeeds)
}
