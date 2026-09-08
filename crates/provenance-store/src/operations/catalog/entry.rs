use super::{ExecutionNeeds, PreparedContext};
use provenance_core::protocol::failure::FailureEnvelope;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
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
    fn needs(request: &Self::Request) -> ExecutionNeeds;
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure>;
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(super) struct DataFreeCall<R> {
    pub request: R,
}

pub(super) struct Entry {
    pub name: &'static str,
    pub invoke: fn(Value) -> OperationFuture<Value, FailureEnvelope>,
    #[cfg(feature = "schema")]
    pub definition: fn() -> super::Definition,
}

fn register<O: Operation>() -> Entry {
    Entry {
        name: O::NAME,
        invoke: super::invoke::invoke_erased::<O>,
        #[cfg(feature = "schema")]
        definition: super::schema::definition::<O>,
    }
}

pub(super) fn entries() -> Vec<Entry> {
    vec![register::<super::CheckStatement>()]
}
