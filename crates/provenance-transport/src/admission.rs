//! Shared bounded admission for ingress and operation work.

use provenance_core::protocol::failure::{ErasedFailure, OperationFailure};
use std::{sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

// Three seconds absorbs short request bursts without an unbounded slot wait.
const WAIT: Duration = Duration::from_secs(3);

pub async fn acquire(semaphore: Arc<Semaphore>) -> Result<OwnedSemaphorePermit, ErasedFailure> {
    // Tokio grants permits in queue order. Closing the semaphore wakes all waiters.
    tokio::time::timeout(WAIT, semaphore.acquire_owned())
        .await
        .map_err(|_| busy())?
        .map_err(|_| busy())
}

pub fn busy() -> ErasedFailure {
    ErasedFailure::new(None, OperationFailure::HostBusy)
}
