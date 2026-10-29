use provenance_core::protocol::failure::{ErasedFailure, OperationFailure};
use provenance_store::operations::catalog::{
    ContextResolver, Direct, ExecutionNeeds, PreparedContext, RequestedContext,
};
use serde_json::Value;

mod dispatch;
mod routes;

/// A resolver with no repositories.
struct NoRepositories;

impl ContextResolver for NoRepositories {
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

/// Runs one call by name on a host with no repositories.
#[allow(clippy::result_large_err)]
async fn invoke(operation: &str, version: u32, call: Value) -> Result<Value, ErasedFailure> {
    crate::invoke_with(
        operation,
        version,
        call,
        Direct(std::sync::Arc::new(NoRepositories)),
    )
    .await
}
