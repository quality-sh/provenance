#![allow(clippy::result_large_err)]

use super::{
    entry::{entries, DataFreeCall, RepositoryCall},
    Operation, OperationFuture, PreparedContext,
};
use provenance_core::{
    protocol::failure::{
        ErasedFailure as FailureEnvelope, InvalidInputReason, OperationError, OperationFailure,
    },
    SDK_PROTOCOL_VERSION,
};
use serde::de::IntoDeserializer as _;
use serde_json::Value;

/// Native callers cross the same preparation and handler seam without JSON conversion.
pub async fn invoke_typed<O: Operation>(
    context: PreparedContext,
    request: O::Request,
) -> Result<O::Success, OperationError<O::Failure>> {
    context
        .validate_kind(O::CONTEXT)
        .map_err(OperationError::Common)?;
    let context = context
        .prepare(O::needs(&request))
        .map_err(OperationError::Common)?;
    O::run(context, request)
        .await
        .map_err(OperationError::Handler)
}

/// Authorize one typed call before it crosses the operation boundary.
pub async fn invoke_authorized_typed<O: Operation>(
    resolver: std::sync::Arc<dyn super::ContextResolver>,
    selected: super::RequestedContext,
    request: O::Request,
) -> Result<O::Success, OperationError<O::Failure>> {
    O::validate_external(&request).map_err(OperationError::Common)?;
    let context = resolver
        .prepare(O::NAME, selected, O::needs(&request))
        .map_err(OperationError::Common)?;
    invoke_typed::<O>(context, request).await
}

pub async fn invoke(operation: &str, version: u32, call: Value) -> Result<Value, FailureEnvelope> {
    invoke_with(
        operation,
        version,
        call,
        std::sync::Arc::new(super::context::NoRepositories),
    )
    .await
}

pub async fn invoke_with(
    operation: &str,
    version: u32,
    call: Value,
    resolver: std::sync::Arc<dyn super::ContextResolver>,
) -> Result<Value, FailureEnvelope> {
    if version != SDK_PROTOCOL_VERSION {
        return Err(FailureEnvelope::new(
            None,
            OperationFailure::ProtocolMismatch {
                requested: version,
                supported: SDK_PROTOCOL_VERSION,
            },
        ));
    }
    let entry = entries()
        .into_iter()
        .find(|entry| entry.name == operation)
        .ok_or_else(|| FailureEnvelope::new(None, OperationFailure::UnknownOperation))?;
    (entry.invoke)(call, resolver).await
}

#[cfg(test)]
pub(super) fn invoke_erased<O: Operation>(call: Value) -> OperationFuture<Value, FailureEnvelope> {
    invoke_resolved::<O>(call, std::sync::Arc::new(super::context::NoRepositories))
}

pub(super) fn invoke_resolved<O: Operation>(
    call: Value,
    resolver: std::sync::Arc<dyn super::ContextResolver>,
) -> OperationFuture<Value, FailureEnvelope> {
    Box::pin(async move {
        let (selected, request) = match O::CONTEXT {
            super::ContextKind::Repository => {
                let call: RepositoryCall<
                    O::Request,
                    provenance_core::protocol::repository::RepositoryTarget,
                > = decode::<O, _>(call)?;
                (
                    Some(super::RequestedContext::Repository(call.context)),
                    call.request,
                )
            }
            super::ContextKind::Scope => {
                let call: RepositoryCall<
                    O::Request,
                    provenance_core::protocol::repository::RepositoryScope,
                > = decode::<O, _>(call)?;
                (
                    Some(super::RequestedContext::Scope(call.context)),
                    call.request,
                )
            }
            super::ContextKind::Scoped => {
                let call: RepositoryCall<
                    O::Request,
                    provenance_core::protocol::repository::RepositoryContext,
                > = decode::<O, _>(call)?;
                (
                    Some(super::RequestedContext::Scoped(call.context)),
                    call.request,
                )
            }
            super::ContextKind::DataFree => {
                let call: DataFreeCall<O::Request> = decode::<O, _>(call)?;
                (None, call.request)
            }
        };
        let success = match selected {
            Some(context) => invoke_authorized_typed::<O>(resolver, context, request).await,
            None => match O::validate_external(&request) {
                Ok(()) => invoke_typed::<O>(PreparedContext::data_free(), request).await,
                Err(error) => Err(OperationError::Common(error)),
            },
        }
        .map_err(frame_failure::<O>)?;
        serde_json::to_value(success)
            .map_err(|_| FailureEnvelope::new(Some(O::NAME), OperationFailure::Internal))
    })
}

fn decode<O: Operation, T: serde::de::DeserializeOwned>(call: Value) -> Result<T, FailureEnvelope> {
    serde_path_to_error::deserialize(call.into_deserializer()).map_err(|error| {
        let path = error.path().to_string();
        FailureEnvelope::new(
            Some(O::NAME),
            OperationFailure::InvalidInput {
                field: (!path.is_empty() && path != ".").then_some(path),
                reason: InvalidInputReason::InvalidValue,
            },
        )
    })
}

fn frame_failure<O: Operation>(failure: OperationError<O::Failure>) -> FailureEnvelope {
    match failure {
        OperationError::Common(error) => FailureEnvelope::new(Some(O::NAME), error),
        OperationError::Handler(error) => {
            let status = O::failure_status(&error);
            FailureEnvelope::declared(O::NAME, error, status)
        }
    }
}
