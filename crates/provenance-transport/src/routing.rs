use crate::StatementHost;
use axum::http::{HeaderMap, Method};
use provenance_core::protocol::failure::{ErasedFailure, InvalidInputReason, OperationFailure};
use provenance_store::operations::catalog::{self, ContextKind, Definition, HttpMethod};
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::OnceLock};

mod identity;
mod request;
mod response;

pub use request::{decode_body, query};

pub struct Matched {
    pub definition: &'static Definition,
    pub path: BTreeMap<String, String>,
}

struct RouteIndex {
    get: Vec<&'static Definition>,
    post: Vec<&'static Definition>,
    patch: Vec<&'static Definition>,
}

fn index() -> &'static RouteIndex {
    static ROUTES: OnceLock<RouteIndex> = OnceLock::new();
    ROUTES.get_or_init(|| {
        let mut index = RouteIndex {
            get: Vec::new(),
            post: Vec::new(),
            patch: Vec::new(),
        };
        for definition in catalog::definitions() {
            match definition.method {
                HttpMethod::Get => &mut index.get,
                HttpMethod::Post => &mut index.post,
                HttpMethod::Patch => &mut index.patch,
            }
            .push(definition);
        }
        index
    })
}

pub fn find(method: &Method, path: &str) -> Option<Matched> {
    let candidates = match *method {
        Method::GET => &index().get,
        Method::POST => &index().post,
        Method::PATCH => &index().patch,
        _ => return None,
    };
    candidates.iter().copied().find_map(|definition| {
        match_path(definition.path, path).map(|path| Matched { definition, path })
    })
}

fn match_path(pattern: &str, actual: &str) -> Option<BTreeMap<String, String>> {
    let expected = pattern.trim_matches('/').split('/').collect::<Vec<_>>();
    let actual = actual.trim_matches('/').split('/').collect::<Vec<_>>();
    if expected.len() != actual.len() {
        return None;
    }
    let mut values = BTreeMap::new();
    for (expected, actual) in expected.into_iter().zip(actual) {
        if let Some(name) = expected
            .strip_prefix('{')
            .and_then(|part| part.strip_suffix('}'))
        {
            if actual.is_empty() {
                return None;
            }
            values.insert(name.to_owned(), actual.to_owned());
        } else if expected != actual {
            return None;
        }
    }
    Some(values)
}

pub async fn invoke(
    host: &StatementHost,
    matched: &Matched,
    data: Value,
    query: BTreeMap<String, String>,
    headers: &HeaderMap,
) -> Result<(response::Success, Option<String>), ErasedFailure> {
    if !host.advertises(matched.definition.name) {
        return Err(ErasedFailure::new(None, OperationFailure::AccessDenied));
    }
    identity::reject(&data, &matched.path, matched.definition)?;
    let identity = host.bound_identity();
    let bound = request::bind(
        matched.definition,
        &matched.path,
        data,
        &query,
        headers,
        identity.as_ref().map(|(_, scope)| scope.as_str()),
    )?;
    let call = host.bound_call(bound.handler.context, &bound.data)?;
    let value = host
        .invoke_backing(matched.definition.name, bound.handler.operation, call)
        .await?;
    let value = response::success(
        value,
        matched.definition,
        &bound.response,
        bound.query_response,
    )?;
    let etag = if bound.query_response {
        None
    } else {
        matched
            .definition
            .registration
            .controls
            .etag
            .as_ref()
            .map(|binding| -> Result<String, ErasedFailure> {
                let value = value
                    .value()
                    .pointer(&format!("/data{}", binding.pointer))
                    .ok_or_else(|| {
                        ErasedFailure::new(
                            Some(matched.definition.name),
                            OperationFailure::Internal,
                        )
                    })?;
                let text = if binding.numeric {
                    value.as_u64().map(|value| value.to_string())
                } else {
                    value.as_str().map(str::to_owned)
                }
                .ok_or_else(|| {
                    ErasedFailure::new(Some(matched.definition.name), OperationFailure::Internal)
                })?;
                Ok(format!("\"{text}\""))
            })
            .transpose()?
    };
    Ok((value, etag))
}

pub fn context(
    identity: Option<(String, String)>,
    kind: ContextKind,
    request: &Value,
) -> Result<Value, ErasedFailure> {
    match kind {
        ContextKind::DataFree => Ok(json!({"request":request})),
        ContextKind::Repository => identity
            .map(|(repository, _)| json!({"context":{"repository":repository},"request":request}))
            .ok_or_else(|| invalid(None)),
        ContextKind::Scope => identity
            .map(|(repository, scope)| json!({"context":{"repository":repository,"scope":scope},"request":request}))
            .ok_or_else(|| invalid(None)),
        ContextKind::Scoped => identity
            .map(|(repository, scope)| json!({"context":{"repository":repository,"scope":scope},"request":request}))
            .ok_or_else(|| invalid(None)),
    }
}

pub fn invalid(field: Option<&str>) -> ErasedFailure {
    ErasedFailure::new(
        None,
        OperationFailure::InvalidInput {
            field: field.map(str::to_owned),
            reason: InvalidInputReason::InvalidValue,
        },
    )
}

/// A request body that failed JSON parsing reports `malformed_json`, not
/// `invalid_value`; the envelope has not been inspected yet.
pub fn malformed_json() -> ErasedFailure {
    ErasedFailure::new(
        None,
        OperationFailure::InvalidInput {
            field: None,
            reason: InvalidInputReason::MalformedJson,
        },
    )
}
