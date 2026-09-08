use super::{
    entry::{entries, DataFreeCall},
    Operation, OperationFuture, PreparedContext,
};
use provenance_core::{
    protocol::failure::{FailureEnvelope, InvalidInputReason, OperationError, OperationFailure},
    SDK_PROTOCOL_VERSION,
};
use serde::de::IntoDeserializer as _;
use serde_json::Value;

/// Native callers cross the same preparation and handler seam without JSON conversion.
pub async fn invoke_typed<O: Operation>(
    context: PreparedContext,
    request: O::Request,
) -> Result<O::Success, OperationError<O::Failure>> {
    let context = context
        .prepare(O::needs(&request))
        .map_err(OperationError::Common)?;
    O::run(context, request)
        .await
        .map_err(OperationError::Handler)
}

pub async fn invoke(operation: &str, version: u32, call: Value) -> Result<Value, FailureEnvelope> {
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
    (entry.invoke)(call).await
}

pub(super) fn invoke_erased<O: Operation>(call: Value) -> OperationFuture<Value, FailureEnvelope> {
    Box::pin(async move {
        let call: DataFreeCall<O::Request> =
            serde_path_to_error::deserialize(call.into_deserializer()).map_err(|error| {
                let path = error.path().to_string();
                FailureEnvelope::new(
                    Some(O::NAME),
                    OperationFailure::InvalidInput {
                        field: (!path.is_empty() && path != ".").then_some(path),
                        reason: InvalidInputReason::InvalidValue,
                    },
                )
            })?;
        let success = invoke_typed::<O>(PreparedContext::data_free(), call.request)
            .await
            .map_err(|error| frame_failure::<O>(error))?;
        serde_json::to_value(success)
            .map_err(|_| FailureEnvelope::new(Some(O::NAME), OperationFailure::Internal))
    })
}

fn frame_failure<O: Operation>(failure: OperationError<O::Failure>) -> FailureEnvelope {
    let error = match failure {
        OperationError::Common(error) => error,
        // The declared family's serialized shape is the wire shape. A manual
        // conversion cannot change its variant or fields after schema generation.
        OperationError::Handler(error) => {
            known_wire_failure(error).unwrap_or(OperationFailure::Internal)
        }
    };
    FailureEnvelope::new(Some(O::NAME), error)
}

fn known_wire_failure<F: serde::Serialize>(failure: F) -> Option<OperationFailure> {
    let declared = serde_json::to_value(failure).ok()?;
    let wire: OperationFailure = serde_json::from_value(declared.clone()).ok()?;
    (serde_json::to_value(&wire).ok()? == declared).then_some(wire)
}
