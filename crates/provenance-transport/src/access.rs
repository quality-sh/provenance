//! Host access remains explicit even when fixture support is absent.
use axum::http::HeaderMap;
use provenance_core::protocol::failure::OperationFailure;
use provenance_store::operations::catalog::{
    CheckStatement, ContextResolver, ExecutionNeeds, Operation, PreparedContext, RequestedContext,
};

/// The repository binding: which operations the host advertises and which identity it binds.
pub trait HostAccess: ContextResolver {
    fn advertises(&self, operation: &str) -> bool;
    fn bound_identity(&self) -> Option<(String, String)>;
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
