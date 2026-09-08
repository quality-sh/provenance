//! Isolated adapters for the shared operation contract.
//!
//! This crate does not open a listener. The optional fixture binary exists only
//! for contract tests. Production exposure remains subject to the host review.
mod access;
mod execution;
mod failure;
#[cfg(feature = "test-fixture")]
pub mod fixture;
mod http;
mod mcp;
mod mcp_io;

use execution::Execution;
use provenance_core::protocol::failure::{ErasedFailure as FailureEnvelope, OperationFailure};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

pub(crate) const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct StatementHost {
    execution: Execution,
    access: Arc<dyn access::HostAccess>,
    ingress: Arc<Semaphore>,
    stopping: CancellationToken,
}

impl Default for StatementHost {
    fn default() -> Self {
        Self {
            execution: Execution::default(),
            access: Arc::new(access::DataFreeAccess),
            ingress: Arc::new(Semaphore::new(8)),
            stopping: CancellationToken::new(),
        }
    }
}

impl StatementHost {
    #[cfg(feature = "test-fixture")]
    pub fn with_fixture_access(access: fixture::FixtureAccess) -> Self {
        Self {
            access: Arc::new(access),
            ..Self::default()
        }
    }
    pub(crate) fn authenticate(
        &self,
        headers: &axum::http::HeaderMap,
    ) -> Result<(), FailureEnvelope> {
        self.access
            .authenticate(headers)
            .map_err(|error| FailureEnvelope::new(None, error))
    }
    pub(crate) fn advertises(&self, operation: &str) -> bool {
        self.access.advertises(operation)
    }

    fn admit(&self) -> Result<OwnedSemaphorePermit, FailureEnvelope> {
        self.ingress
            .clone()
            .try_acquire_owned()
            .map_err(|_| FailureEnvelope::new(None, OperationFailure::UnavailableNeeds))
    }

    /// Serve a bounded MCP session on a caller-owned test stream.
    pub async fn serve_mcp<T>(
        self,
        io: T,
    ) -> Result<
        rmcp::service::RunningService<rmcp::RoleServer, Self>,
        rmcp::service::ServerInitializeError,
    >
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        rmcp::ServiceExt::serve(self, mcp_io::BoundedIo::new(io)).await
    }

    pub fn router(&self) -> axum::Router {
        http::router(self.clone())
    }

    /// Close admission and wait for all operation work, including disconnected calls.
    pub async fn shutdown(&self) {
        self.ingress.close();
        self.stopping.cancel();
        self.execution.shutdown().await;
    }

    async fn invoke(
        &self,
        operation: String,
        version: u32,
        call: Value,
    ) -> Result<Value, FailureEnvelope> {
        let runtime = tokio::runtime::Handle::current();
        let access: Arc<dyn provenance_store::operations::catalog::ContextResolver> =
            self.access.clone();
        let dispatched_operation = operation.clone();
        self.execution
            .run(&operation, move || {
                runtime.block_on(provenance_store::operations::catalog::invoke_with(
                    &dispatched_operation,
                    version,
                    call,
                    access,
                ))
            })
            .await
    }
}
