use crate::{failure, routing, StatementHost, MAX_BODY_BYTES};
use axum::{
    body::{to_bytes, Bytes},
    extract::{Request, State},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use provenance_core::protocol::{
    failure::{ErasedFailure, InvalidInputReason, OperationFailure},
    host::HostMetadata,
    ResponseMeta, SuccessEnvelope,
};
use provenance_macros::rule;
use provenance_store::operations::catalog::{self, HttpMethod};

pub fn router(host: StatementHost) -> Router {
    let mut router = Router::new().route("/metadata", get(metadata));
    for definition in catalog::definitions() {
        let path = axum_path(definition.path);
        router = match definition.method {
            HttpMethod::Get => router.route(&path, get(invoke)),
            HttpMethod::Post => router.route(&path, post(invoke)),
            HttpMethod::Patch => router.route(&path, patch(invoke)),
        };
    }
    router
        .method_not_allowed_fallback(method_not_allowed)
        .fallback(unknown)
        .with_state(host)
}

fn axum_path(path: &str) -> String {
    path.split('/')
        .enumerate()
        .map(|(index, part)| {
            part.strip_prefix('{')
                .and_then(|part| part.strip_suffix('}'))
                .map_or_else(|| part.to_owned(), |_| format!(":p{index}"))
        })
        .collect::<Vec<_>>()
        .join("/")
}

async fn unknown(State(host): State<StatementHost>, request: Request) -> Response {
    if let Err(error) = host.authenticate(request.headers()) {
        return failure::response(error);
    }
    failure::response(ErasedFailure::new(None, OperationFailure::UnknownOperation))
}

async fn method_not_allowed(State(host): State<StatementHost>, request: Request) -> Response {
    if let Err(error) = host.authenticate(request.headers()) {
        return failure::response(error);
    }
    failure::response(ErasedFailure::new(None, OperationFailure::MethodNotAllowed))
}

async fn metadata(State(host): State<StatementHost>, request: Request) -> Response {
    if let Err(error) = host.authenticate(request.headers()) {
        return failure::response(error);
    }
    let (repository, scope) = host
        .bound_identity()
        .map_or((None, None), |(r, s)| (Some(r), Some(s)));
    Json(SuccessEnvelope {
        data: HostMetadata::current(repository, scope),
        meta: ResponseMeta::default(),
    })
    .into_response()
}

async fn invoke(State(host): State<StatementHost>, request: Request) -> Response {
    if let Err(error) = host.authenticate(request.headers()) {
        return failure::response(error);
    }
    crate::admission::with_deadline(invoke_admitted(host, request)).await
}

async fn invoke_admitted(host: StatementHost, request: Request) -> Response {
    let _admission = match host.admit().await {
        Ok(permit) => permit,
        Err(error) => return failure::response(error),
    };
    let Some(matched) = routing::find(request.method(), request.uri().path()) else {
        return failure::response(ErasedFailure::new(None, OperationFailure::UnknownOperation));
    };
    if !host.advertises(matched.definition.name) {
        return failure::response(ErasedFailure::new(None, OperationFailure::AccessDenied));
    }
    let query = match routing::query(request.uri().query()) {
        Ok(query) => query,
        Err(error) => return failure::response(error),
    };
    let page_write = request
        .extensions()
        .get::<crate::ReviewPageWrite>()
        .is_some()
        && matched.definition.mutates();
    let headers = request.headers().clone();
    let expects_body = matched.definition.request_schema().is_some();
    let bytes = match read_body(&host, request).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };
    let data = match routing::decode_body(&bytes, expects_body) {
        Ok(data) => data,
        Err(error) => return failure::response(error),
    };
    match routing::invoke(&host, &matched, data, query, &headers, page_write).await {
        Ok((value, etag)) => {
            let mut response = (
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                value.into_bytes(),
            )
                .into_response();
            if let Some(etag) = etag.and_then(|etag| etag.parse().ok()) {
                response.headers_mut().insert("etag", etag);
            }
            response
        }
        Err(error) => failure::response(error),
    }
}

/// Reads the body until the request deadline, so a stalled upload cannot hold its slot.
///
/// The slot wait and the full upload share the three-second deadline. When the host stops,
/// the read ends with `unavailable_needs`. A body that is not complete at the deadline gets
/// `invalid_input` with the reason `incomplete`.
#[rule("rule_host_body_read_bound")]
async fn read_body(host: &StatementHost, request: Request) -> Result<Bytes, Response> {
    let deadline = crate::admission::deadline();
    tokio::select! {
        biased;
        () = host.stopping.cancelled() => Err(failure::response(crate::admission::unavailable())),
        read = tokio::time::timeout_at(deadline, to_bytes(request.into_body(), MAX_BODY_BYTES)) => match read {
            Ok(Ok(bytes)) => Ok(bytes),
            Ok(Err(_)) => Err(invalid(InvalidInputReason::TooLarge)),
            Err(_) => Err(invalid(InvalidInputReason::Incomplete)),
        },
    }
}

fn invalid(reason: InvalidInputReason) -> Response {
    failure::response(ErasedFailure::new(
        None,
        OperationFailure::InvalidInput {
            field: None,
            reason,
        },
    ))
}
