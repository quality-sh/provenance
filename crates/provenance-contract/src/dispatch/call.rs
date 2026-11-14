//! The native call of one operation: its context, by the context kind of the operation, and its
//! request.

use super::decode;
use provenance_core::protocol::failure::ErasedFailure as FailureEnvelope;
use provenance_core::protocol::repository::{RepositoryContext, RepositoryScope, RepositoryTarget};
use provenance_store::operations::catalog::{ContextKind, Operation, RequestedContext};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DataFreeCall<R> {
    request: R,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryCall<R, C> {
    context: C,
    request: R,
}

/// The selected context and the request of the native call of one operation.
pub(crate) fn decode_call<O: Operation>(
    call: Value,
) -> Result<(Option<RequestedContext>, O::Request), FailureEnvelope> {
    Ok(match O::CONTEXT {
        ContextKind::Repository => {
            let call: RepositoryCall<O::Request, RepositoryTarget> = decode::<O, _>(call)?;
            (
                Some(RequestedContext::Repository(call.context)),
                call.request,
            )
        }
        ContextKind::Scope => {
            let call: RepositoryCall<O::Request, RepositoryScope> = decode::<O, _>(call)?;
            (Some(RequestedContext::Scope(call.context)), call.request)
        }
        ContextKind::Scoped => {
            let call: RepositoryCall<O::Request, RepositoryContext> = decode::<O, _>(call)?;
            (Some(RequestedContext::Scoped(call.context)), call.request)
        }
        ContextKind::DataFree => {
            let call: DataFreeCall<O::Request> = decode::<O, _>(call)?;
            (None, call.request)
        }
    })
}
