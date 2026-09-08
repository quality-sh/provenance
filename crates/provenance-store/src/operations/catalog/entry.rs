use super::{ExecutionNeeds, PreparedContext};
use provenance_core::protocol::failure::ErasedFailure as FailureEnvelope;
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

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(super) struct DataFreeCall<R> {
    pub request: R,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(super) struct RepositoryCall<R, C> {
    pub context: C,
    pub request: R,
}

pub(super) struct Entry {
    pub name: &'static str,
    pub mutates: bool,
    pub invoke: fn(
        Value,
        std::sync::Arc<dyn super::ContextResolver>,
    ) -> OperationFuture<Value, FailureEnvelope>,
    #[cfg(feature = "schema")]
    pub definition: fn() -> super::Definition,
}

fn register<O: Operation>() -> Entry {
    Entry {
        name: O::NAME,
        mutates: O::MUTATES,
        invoke: super::invoke::invoke_resolved::<O>,
        #[cfg(feature = "schema")]
        definition: super::schema::definition::<O>,
    }
}

pub(super) fn entries() -> Vec<Entry> {
    vec![
        register::<super::CheckStatement>(),
        register::<super::CreateSource>(),
        register::<super::CreateRequirement>(),
        register::<super::CreateRule>(),
        register::<super::CreateResolution>(),
        register::<super::AddSourceReference>(),
        register::<super::Plan>(),
        register::<super::Apply>(),
        register::<super::BeginVerification>(),
        register::<super::CompleteVerification>(),
        register::<super::Info>(),
        register::<super::Get>(),
        register::<super::Search>(),
        register::<super::Neighbors>(),
        register::<super::Trace>(),
        register::<super::Impact>(),
        register::<super::ResolveSymbol>(),
        register::<super::Evidence>(),
        register::<super::Stale>(),
        register::<super::VerificationRuns>(),
        register::<super::VerificationBindings>(),
        register::<super::ListThreads>(),
        register::<super::ListMessages>(),
        register::<super::PostThreadMessage>(),
    ]
}
