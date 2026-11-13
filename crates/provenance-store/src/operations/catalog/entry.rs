//! The typed operation contract: the wire contract in `provenance-contract` names these
//! types. Add no store state here.
use super::{ExecutionNeeds, PreparedContext, RequestedContext};
use provenance_core::protocol::failure::{OperationError, OperationFailure};
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

/// A failure that only the handler of an operation gives.
pub trait HandlerFailure: std::error::Error + Serialize + WireSchema + Send + 'static {
    /// The response status of this failure.
    fn failure_status(&self) -> u16;
    /// The native cause of a failure of no known kind, kept out of the wire answer.
    fn log_cause(&self) -> Option<String> {
        None
    }
}

/// Each failure that a call of one operation gives. `OperationFailure` is this set for an
/// operation whose handler gives no failure. `OperationError` adds the failures of a
/// handler to it.
pub trait CallFailure:
    From<OperationFailure> + std::error::Error + Serialize + WireSchema + Send + 'static
{
    /// The response status of this failure.
    fn failure_status(&self) -> u16;
    /// The native cause of a failure of no known kind, kept out of the wire answer.
    fn log_cause(&self) -> Option<String>;
}

impl CallFailure for OperationFailure {
    fn failure_status(&self) -> u16 {
        self.status_code()
    }
    fn log_cause(&self) -> Option<String> {
        None
    }
}

impl<F: HandlerFailure> CallFailure for OperationError<F> {
    fn failure_status(&self) -> u16 {
        match self {
            Self::Common(failure) => failure.status_code(),
            Self::Handler(failure) => failure.failure_status(),
        }
    }
    fn log_cause(&self) -> Option<String> {
        match self {
            Self::Common(_) => None,
            Self::Handler(failure) => failure.log_cause(),
        }
    }
}

/// The associated types bind wire definitions to the invoked handler.
pub trait Operation: Send + Sync + 'static {
    /// The unit type is the request of an operation that takes no input.
    type Request: DeserializeOwned + WireSchema + Send + 'static;
    type Success: Serialize + WireSchema + Send + 'static;
    /// Each failure that a call gives. The wire contract publishes the schema of this type.
    type Failure: CallFailure;
    const NAME: &'static str;
    const MUTATES: bool = false;
    const CONTEXT: super::ContextKind = super::ContextKind::DataFree;
    const FAILURE_STATUSES: &'static [u16] = &[];
    fn validate_external(_: &Self::Request) -> Result<(), OperationFailure> {
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
    ) -> OperationFuture<O::Success, O::Failure>;
}
