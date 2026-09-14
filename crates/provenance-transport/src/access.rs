//! Host access remains explicit even when fixture support is absent.
use axum::http::HeaderMap;
use provenance_core::protocol::failure::OperationFailure;
use provenance_store::operations::catalog::{
    CheckStatement, ContextResolver, ExecutionNeeds, Operation, PreparedContext, RequestedContext,
};

pub trait HostAccess: ContextResolver {
    fn authenticate(&self, headers: &HeaderMap) -> Result<(), OperationFailure>;
    fn advertises(&self, operation: &str) -> bool;
    fn bound_identity(&self) -> Option<(String, String)>;
}

pub struct DataFreeAccess;

impl HostAccess for DataFreeAccess {
    fn authenticate(&self, _: &HeaderMap) -> Result<(), OperationFailure> {
        Ok(())
    }

    fn advertises(&self, operation: &str) -> bool {
        operation == CheckStatement::NAME
    }

    fn bound_identity(&self) -> Option<(String, String)> {
        None
    }
}

impl ContextResolver for DataFreeAccess {
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
    fn authenticate(&self, headers: &HeaderMap) -> Result<(), OperationFailure> {
        self.authenticate(headers)
    }

    fn advertises(&self, operation: &str) -> bool {
        self.permits_operation(operation)
    }

    fn bound_identity(&self) -> Option<(String, String)> {
        self.bound_identity()
    }
}
