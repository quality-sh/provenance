#![allow(clippy::result_large_err)]

//! This crate is an implementation part of Provenance. Provenance is pre-release, and this API can
//! change without compatibility shims. Use `provenance-sdk` for Rust.
//!
//! Isolated adapters for the shared operation contract.
//!
//! This library does not open a listener. The `provenance-review` crate owns the local review
//! listener.
mod access;
mod atomic_file;
mod admission;
#[cfg(test)]
mod admission_tests;
pub mod check;
pub mod coverage;
pub mod docs;
mod execution;
mod failure;
mod gitignore;
#[cfg(feature = "test-fixture")]
pub mod fixture;
mod http;
mod init_summary;
mod invocation;
mod legacy_cleanup;
mod local;
pub mod local_host;
mod onboarding;
pub mod porcelain;
pub mod repository;
mod review_page;
pub mod reviewer;
mod routing;
pub mod skills;
pub mod snapshot;
mod ste_onboarding;
pub use review_page::ReviewPageWrite;

pub use access::{HostAccess, HttpCredential, OpenCredential};
pub use invocation::Invocation;
pub use local::{constant_time_eq, LocalAccess};

use execution::Execution;
use provenance_core::protocol::failure::{ErasedFailure as FailureEnvelope, OperationFailure};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

/// The largest request body that the HTTP adapter reads.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct StatementHost {
    execution: Execution,
    page_writes: Option<review_page::PageWrites>,
    access: Arc<dyn access::HostAccess>,
    credential: Option<Arc<dyn access::HttpCredential>>,
    check_port: Option<Arc<dyn provenance_porcelain::check::CheckPort>>,
    notify_port: Option<Arc<dyn provenance_porcelain::get::NotifyPort>>,
    review_link_port: Option<Arc<dyn provenance_porcelain::get::ReviewLinkPort>>,
    repository: Option<repository::RepoContext>,
    ingress: Arc<Semaphore>,
    stopping: CancellationToken,
}

impl Default for StatementHost {
    fn default() -> Self {
        Self {
            execution: Execution::default(),
            page_writes: None,
            access: Arc::new(access::DataFreeAccess),
            credential: Some(Arc::new(access::OpenCredential)),
            check_port: None,
            notify_port: None,
            review_link_port: None,
            repository: None,
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

    /// Bind a repository. The host has no credential policy until `with_credential` names
    /// one, so its router refuses every request.
    pub fn with_access(access: Arc<dyn HostAccess>) -> Self {
        Self {
            access,
            credential: None,
            ..Self::default()
        }
    }
    /// The transport policy an HTTP request must satisfy.
    #[must_use]
    pub fn with_credential(mut self, credential: Arc<dyn HttpCredential>) -> Self {
        self.credential = Some(credential);
        self
    }
    /// Observes only changes from HTTP requests marked by the owning review page host.
    #[must_use]
    pub fn with_page_writes(
        mut self,
        repository: provenance_store::repository::Repository,
        observed: Arc<
            dyn Fn(anyhow::Result<provenance_store::review_changes::ReviewChanges>) + Send + Sync,
        >,
    ) -> Self {
        self.page_writes = Some(review_page::PageWrites {
            repository,
            observed,
        });
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

    /// Inject the review host client used by the get `notify` argument.
    #[must_use]
    pub fn with_notify_port(
        mut self,
        port: Arc<dyn provenance_porcelain::get::NotifyPort>,
    ) -> Self {
        self.notify_port = Some(port);
        self
    }

    /// Inject the review host client used by the get `review_link` argument.
    #[must_use]
    pub fn with_review_link_port(
        mut self,
        port: Arc<dyn provenance_porcelain::get::ReviewLinkPort>,
    ) -> Self {
        self.review_link_port = Some(port);
        self
    }

    /// Bind the repository files that materialize, import, dictionary import, and
    /// swarm-backtrace land work on.
    #[must_use]
    pub fn with_repository(mut self, context: repository::RepoContext) -> Self {
        self.repository = Some(context);
        self
    }

    pub(crate) const fn repository(&self) -> Option<&repository::RepoContext> {
        self.repository.as_ref()
    }

    pub(crate) fn review_link_port(
        &self,
    ) -> Option<&Arc<dyn provenance_porcelain::get::ReviewLinkPort>> {
        self.review_link_port.as_ref()
    }

    pub(crate) fn notify_port(&self) -> Option<&Arc<dyn provenance_porcelain::get::NotifyPort>> {
        self.notify_port.as_ref()
    }

    /// The repository-aware computations that the `check` command uses, when injected.
    pub fn check_port(&self) -> Option<&Arc<dyn provenance_porcelain::check::CheckPort>> {
        self.check_port.as_ref()
    }
    /// The fixture principal is both the repository binding and the HTTP credential.
    #[cfg(feature = "test-fixture")]
    pub fn with_fixture_access(access: fixture::FixtureAccess) -> Self {
        let access = Arc::new(access);
        Self {
            access: access.clone(),
            credential: Some(access),
            ..Self::default()
        }
    }
    /// A host without a credential policy refuses every HTTP request.
    pub(crate) fn authenticate(
        &self,
        headers: &axum::http::HeaderMap,
    ) -> Result<(), FailureEnvelope> {
        self.credential
            .as_ref()
            .ok_or(OperationFailure::Unauthenticated)
            .and_then(|credential| credential.authenticate(headers))
            .map_err(|error| FailureEnvelope::new(None, error))
    }
    pub(crate) fn advertises(&self, operation: &str) -> bool {
        self.access.advertises(operation)
    }
    /// The repository target and scope that the host access binds, when it binds one.
    pub fn bound_identity(&self) -> Option<(String, String)> {
        self.access.bound_identity()
    }

    async fn admit(&self) -> Result<OwnedSemaphorePermit, FailureEnvelope> {
        tokio::select! {
            biased;
            () = self.stopping.cancelled() => Err(admission::unavailable()),
            permit = admission::acquire(self.ingress.clone()) => permit,
        }
    }

    /// Run one interface call under the request deadline while it holds an admission slot.
    ///
    /// A closed or busy host refuses the call before `work` starts.
    pub async fn admitted<F: std::future::Future>(
        &self,
        work: F,
    ) -> Result<F::Output, FailureEnvelope> {
        admission::with_deadline(async {
            let _admission = self.admit().await?;
            Ok(work.await)
        })
        .await
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
