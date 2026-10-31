//! The typed operation contract: the wire contract in `provenance-contract` names these
//! types. Add no store state here.
use super::{ExecutionNeeds, PreparedContext, RequestedContext};
use provenance_core::protocol::failure::OperationError;
use serde::{de::DeserializeOwned, Serialize};
use std::{future::Future, pin::Pin};

pub type OperationFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send>>;

#[cfg(feature = "schema")]
pub trait WireSchema: schemars::JsonSchema {}
#[cfg(feature = "schema")]
impl<T: schemars::JsonSchema> WireSchema for T {}
#[cfg(not(feature = "schema"))]
pub trait WireSchema {}
#[cfg(not(feature = "schema"))]
impl<T> WireSchema for T {}

/// The associated types bind wire definitions to the invoked handler.
pub trait Operation: Send + Sync + 'static {
    type Request: DeserializeOwned + WireSchema + Send + 'static;
    type Success: Serialize + WireSchema + Send + 'static;
    type Failure: std::error::Error + Serialize + WireSchema + Send + 'static;
    const NAME: &'static str;
    const MUTATES: bool = false;
    const CONTEXT: super::ContextKind = super::ContextKind::DataFree;
    const FAILURE_STATUSES: &'static [u16] = &[];
    fn failure_status(_: &Self::Failure) -> u16 {
        500
    }
    fn validate_external(
        _: &Self::Request,
    ) -> Result<(), provenance_core::protocol::failure::OperationFailure> {
        Ok(())
    }
    fn needs(request: &Self::Request) -> ExecutionNeeds;
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure>;
}

/// A host that runs one typed operation; the contract dispatch decodes and encodes JSON.
pub trait Invoke: Clone + Send + Sync + 'static {
    fn invoke<O: Operation>(
        self,
        selected: Option<RequestedContext>,
        request: O::Request,
    ) -> OperationFuture<O::Success, OperationError<O::Failure>>;
}
