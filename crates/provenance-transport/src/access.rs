//! Host access remains explicit even when fixture support is absent.
use axum::http::HeaderMap;
use provenance_core::protocol::failure::OperationFailure;
use provenance_store::operations::catalog::{
    CheckStatement, ContextResolver, ExecutionNeeds, Operation, PreparedContext, RequestedContext,
};
use provenance_store::operations::read_operations::ReadOperations;
use std::sync::Arc;

/// The repository binding: which operations the host advertises and which identity it binds.
pub trait HostAccess: ContextResolver {
    fn advertises(&self, operation: &str) -> bool;
    fn bound_identity(&self) -> Option<(String, String)>;
    /// The read operations port of the bound repository, if it has one.
    ///
    /// The router sends each read operation to it. Before it does, the call has the checks,
    /// the authorization, the work slot and the preparation through `prepare` of an operation
    /// that the router runs itself, in the same order. A refusal of `prepare` is the answer, and
    /// the port is not asked. The answer of the port is checked against the operation.
    fn read_operations(&self) -> Option<Arc<dyn ReadOperations>> {
        None
    }
}

/// The transport policy an HTTP request must satisfy before the host classifies it.
pub trait HttpCredential: Send + Sync {
    fn authenticate(&self, headers: &HeaderMap) -> Result<(), OperationFailure>;
}

/// The explicit policy of the data-free host: every caller is accepted.
pub struct OpenCredential;

impl HttpCredential for OpenCredential {
    fn authenticate(&self, _: &HeaderMap) -> Result<(), OperationFailure> {
        Ok(())
    }
}

pub struct DataFreeAccess;

impl HostAccess for DataFreeAccess {
    fn advertises(&self, operation: &str) -> bool {
        operation == CheckStatement::NAME
    }

    fn bound_identity(&self) -> Option<(String, String)> {
        None
    }
}

impl ContextResolver for DataFreeAccess {
    fn authorize(&self, _: &'static str, _: &RequestedContext) -> Result<(), OperationFailure> {
        Err(OperationFailure::UnavailableNeeds)
    }

    fn prepare(
        &self,
        _: &'static str,
        _: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        Err(OperationFailure::UnavailableNeeds)
    }
}

#[cfg(feature = "test-fixture")]
impl HostAccess for crate::fixture::FixtureAccess {
    fn advertises(&self, operation: &str) -> bool {
        self.permits_operation(operation)
    }

    fn bound_identity(&self) -> Option<(String, String)> {
        self.bound_identity()
    }
}
