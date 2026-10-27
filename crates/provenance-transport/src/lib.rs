#![allow(clippy::result_large_err)]

//! This crate is an implementation part of Provenance. Provenance is pre-release, and this API can
//! change without compatibility shims. Use `provenance-sdk` for Rust.
//!
//! Isolated adapters for the shared operation contract.
//!
//! This library does not open a listener. The CLI owns the local review listener.
mod access;
mod admission;
#[cfg(test)]
mod admission_tests;
mod execution;
mod failure;
#[cfg(feature = "test-fixture")]
pub mod fixture;
mod http;
mod invocation;
mod local;
pub mod local_host;
mod mcp;
mod mcp_io;
mod mcp_surface;
pub mod porcelain;
mod review_page;
mod routing;
pub use review_page::ReviewPageWrite;

pub use access::HostAccess;
pub use invocation::Invocation;
pub use local::{constant_time_eq, LocalAccess};

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
    page_writes: Option<review_page::PageWrites>,
    access: Arc<dyn access::HostAccess>,
    check_port: Option<Arc<dyn provenance_porcelain::check::CheckPort>>,
    ingress: Arc<Semaphore>,
    stopping: CancellationToken,
}

impl Default for StatementHost {
    fn default() -> Self {
        Self {
            execution: Execution::default(),
            page_writes: None,
            access: Arc::new(access::DataFreeAccess),
            check_port: None,
            ingress: Arc::new(Semaphore::new(8)),
            stopping: CancellationToken::new(),
        }
    }
}

impl StatementHost {
    /// Construct Porcelain capabilities with this host's matching adapters.
    pub const fn porcelain(&self) -> porcelain::HostPorcelain<'_> {
        porcelain::HostPorcelain::new(self)
    }

    /// Use an explicit caller and repository access policy.
    pub fn with_access(access: Arc<dyn HostAccess>) -> Self {
        Self {
            access,
            ..Self::default()
        }
    }
    /// Observes only changes from HTTP requests marked by the owning review page host.
    #[must_use]
    pub fn with_page_writes(
        mut self,
        layout: provenance_store::layout::ProvenanceLayout,
        observed: Arc<
            dyn Fn(anyhow::Result<provenance_store::review_changes::ReviewChanges>) + Send + Sync,
        >,
    ) -> Self {
        self.page_writes = Some(review_page::PageWrites { layout, observed });
        self
    }
    /// Inject the repository-aware computations used by the MCP `check` tool.
    #[must_use]
    pub fn with_check_port(
        mut self,
        port: Arc<dyn provenance_porcelain::check::CheckPort>,
    ) -> Self {
        self.check_port = Some(port);
        self
    }

    pub(crate) fn check_port(&self) -> Option<&Arc<dyn provenance_porcelain::check::CheckPort>> {
        self.check_port.as_ref()
    }
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
    pub(crate) fn bound_identity(&self) -> Option<(String, String)> {
        self.access.bound_identity()
    }

    async fn admit(&self) -> Result<OwnedSemaphorePermit, FailureEnvelope> {
        tokio::select! {
            biased;
            () = self.stopping.cancelled() => Err(admission::unavailable()),
            permit = admission::acquire(self.ingress.clone()) => permit,
        }
    }

    /// Serve a bounded MCP session on a caller-owned test stream.
    ///
    /// The library's initialization error is a large enum, so the public
    /// failure channel boxes it to keep this result small.
    pub async fn serve_mcp<T>(
        self,
        io: T,
    ) -> Result<
        rmcp::service::RunningService<rmcp::RoleServer, Self>,
        Box<rmcp::service::ServerInitializeError>,
    >
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        rmcp::ServiceExt::serve(self, mcp_io::BoundedIo::new(io))
            .await
            .map_err(Box::new)
    }

    pub fn router(&self) -> axum::Router {
        http::router(self.clone())
    }

    /// Invoke one registered resource route for a native caller that already
    /// bound its repository and scope through `HostAccess`.
    ///
    /// A path the catalog publishes under another method refuses with the
    /// canonical `method_not_allowed`; anything else is `unknown_operation`,
    /// matching the public HTTP router.
    pub async fn invoke_resource(
        &self,
        method: axum::http::Method,
        path: &str,
        data: Value,
        query: std::collections::BTreeMap<String, String>,
        headers: axum::http::HeaderMap,
    ) -> Result<Value, FailureEnvelope> {
        let matched = routing::find(&method, path).ok_or_else(|| {
            let failure = if routing::path_is_known(path) {
                OperationFailure::MethodNotAllowed
            } else {
                OperationFailure::UnknownOperation
            };
            FailureEnvelope::new(None, failure)
        })?;
        routing::invoke(self, &matched, data, query, &headers, false)
            .await
            .map(|result| result.0.into_value())
    }

    /// Close admission and wait for all operation work, including disconnected calls.
    #[provenance_macros::rule("rule_host_shutdown_completes_started_work")]
    pub async fn shutdown(&self) {
        self.ingress.close();
        self.stopping.cancel();
        self.execution.shutdown().await;
    }
}
