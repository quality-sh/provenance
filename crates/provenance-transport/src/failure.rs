use axum::response::{IntoResponse, Response};
use provenance_core::protocol::failure::FailureEnvelope;

pub fn response(failure: FailureEnvelope) -> Response {
    (
        axum::http::StatusCode::from_u16(failure.error.status_code())
            .expect("valid contract status"),
        axum::Json(failure),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use provenance_core::protocol::failure::{InvalidInputReason, OperationFailure};
    #[test]
    fn all_common_refusals_map_to_the_advertised_status() {
        for (failure, status) in [
            (
                OperationFailure::InvalidInput {
                    field: None,
                    reason: InvalidInputReason::Required,
                },
                400,
            ),
            (
                OperationFailure::ProtocolMismatch {
                    requested: 6,
                    supported: 7,
                },
                400,
            ),
            (OperationFailure::UnknownOperation, 404),
            (OperationFailure::Unauthenticated, 401),
            (OperationFailure::AccessDenied, 403),
            (OperationFailure::UnknownTarget, 404),
            (OperationFailure::UnavailableNeeds, 503),
            (OperationFailure::Internal, 500),
        ] {
            assert_eq!(
                response(FailureEnvelope::new(None, failure))
                    .status()
                    .as_u16(),
                status
            );
        }
    }
}
