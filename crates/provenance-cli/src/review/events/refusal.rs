//! The refusal of a notify request: a request field to change, a failure, or a busy host.
use axum::{
    response::{IntoResponse, Response},
    Json,
};

/// A refused registration names the request field to change; a failed one has no field.
/// A busy one can run again without a change.
pub(super) struct Refusal {
    field: Option<&'static str>,
    busy: bool,
    message: String,
}

impl Refusal {
    pub(super) fn input(field: &'static str, message: &str) -> Self {
        Self {
            field: Some(field),
            busy: false,
            message: message.to_owned(),
        }
    }

    pub(super) const fn failed(message: String) -> Self {
        Self {
            field: None,
            busy: false,
            message,
        }
    }

    pub(super) fn busy(message: &str) -> Self {
        Self {
            field: None,
            busy: true,
            message: message.to_owned(),
        }
    }
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        let (status, error) = match self.field {
            None if self.busy => (
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                serde_json::json!({"kind": "host_busy", "message": self.message}),
            ),
            Some(field) => (
                axum::http::StatusCode::BAD_REQUEST,
                serde_json::json!({"kind": "invalid_input", "field": field,
                    "reason": "invalid_value", "message": self.message}),
            ),
            None => (
                axum::http::StatusCode::BAD_GATEWAY,
                serde_json::json!({"kind": "operation_failed", "message": self.message}),
            ),
        };
        (
            status,
            Json(serde_json::json!({"error": error, "meta": {}})),
        )
            .into_response()
    }
}
