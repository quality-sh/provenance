use crate::{failure, StatementHost, MAX_BODY_BYTES};
use axum::{
    body::to_bytes,
    extract::{Path, Request, State},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use provenance_core::protocol::{
    failure::{FailureEnvelope, InvalidInputReason, OperationFailure},
    host::HostMetadata,
};
use provenance_store::operations::catalog;

pub fn router(host: StatementHost) -> Router {
    Router::new()
        .route("/metadata", get(|| async { Json(HostMetadata::current()) }))
        .route("/:version/operations/:operation", post(invoke))
        .fallback(|| async {
            failure::response(FailureEnvelope::new(
                None,
                OperationFailure::UnknownOperation,
            ))
        })
        .with_state(host)
}

async fn invoke(
    State(host): State<StatementHost>,
    Path((version, operation)): Path<(String, String)>,
    request: Request,
) -> Response {
    let _admission = match host.admit() {
        Ok(permit) => permit,
        Err(error) => return failure::response(error),
    };
    let Some(version) = version
        .strip_prefix('v')
        .and_then(|v| v.parse::<u32>().ok())
    else {
        return invalid(
            None,
            Some("protocol_version"),
            InvalidInputReason::InvalidValue,
        );
    };
    if !catalog::definitions()
        .iter()
        .any(|entry| entry.name == operation)
    {
        return failure::response(FailureEnvelope::new(
            None,
            OperationFailure::UnknownOperation,
        ));
    }
    if version != provenance_core::SDK_PROTOCOL_VERSION {
        return failure::response(FailureEnvelope::new(
            None,
            OperationFailure::ProtocolMismatch {
                requested: version,
                supported: provenance_core::SDK_PROTOCOL_VERSION,
            },
        ));
    }
    let Ok(bytes) = (tokio::select! {
        biased;
        () = host.stopping.cancelled() => {
            return failure::response(FailureEnvelope::new(None, OperationFailure::UnavailableNeeds));
        }
        bytes = to_bytes(request.into_body(), MAX_BODY_BYTES) => bytes,
    }) else {
        return invalid(Some(&operation), None, InvalidInputReason::TooLarge);
    };
    let Ok(call) = serde_json::from_slice(&bytes) else {
        return invalid(Some(&operation), None, InvalidInputReason::MalformedJson);
    };
    match host.invoke(operation, version, call).await {
        Ok(value) => Json(value).into_response(),
        Err(error) => failure::response(error),
    }
}

fn invalid(operation: Option<&str>, field: Option<&str>, reason: InvalidInputReason) -> Response {
    failure::response(FailureEnvelope::new(
        operation,
        OperationFailure::InvalidInput {
            field: field.map(str::to_owned),
            reason,
        },
    ))
}
