use crate::{failure, routing, StatementHost, MAX_BODY_BYTES};
use axum::{
    body::to_bytes,
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
    router.fallback(unknown).with_state(host)
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

async fn unknown() -> Response {
    failure::response(ErasedFailure::new(None, OperationFailure::UnknownOperation))
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
    let _admission = match host.admit() {
        Ok(permit) => permit,
        Err(error) => return failure::response(error),
    };
    let Some(matched) = routing::find(request.method(), request.uri().path()) else {
        return failure::response(ErasedFailure::new(None, OperationFailure::UnknownOperation));
    };
    let query = match routing::query(request.uri().query()) {
        Ok(query) => query,
        Err(error) => return failure::response(error),
    };
    let headers = request.headers().clone();
    let expects_body = matched.definition.request_schema.is_some();
    let Ok(bytes) = (tokio::select! {
        biased;
        () = host.stopping.cancelled() => return failure::response(ErasedFailure::new(None, OperationFailure::UnavailableNeeds)),
        bytes = to_bytes(request.into_body(), MAX_BODY_BYTES) => bytes,
    }) else {
        return invalid(InvalidInputReason::TooLarge);
    };
    let data = match routing::decode_body(&bytes, expects_body) {
        Ok(data) => data,
        Err(error) => return failure::response(error),
    };
    match routing::invoke(&host, &matched, data, query, &headers).await {
        Ok((value, etag)) => {
            let mut response = Json(value).into_response();
            if let Some(etag) = etag.and_then(|etag| etag.parse().ok()) {
                response.headers_mut().insert("etag", etag);
            }
            response
        }
        Err(error) => failure::response(error),
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
