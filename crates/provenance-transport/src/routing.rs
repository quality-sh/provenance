use crate::StatementHost;
use axum::http::{HeaderMap, Method};
use provenance_core::protocol::failure::{ErasedFailure, InvalidInputReason, OperationFailure};
use provenance_store::operations::catalog::{self, ContextKind, Definition, HttpMethod};
use serde_json::{json, Value};
use std::collections::BTreeMap;

mod identity;
mod request;
mod response;

pub use request::{decode_body, query};

pub struct Matched {
    pub definition: Definition,
    pub path: BTreeMap<String, String>,
}

pub fn find(method: &Method, path: &str) -> Option<Matched> {
    let wanted = match *method {
        Method::GET => HttpMethod::Get,
        Method::POST => HttpMethod::Post,
        Method::PATCH => HttpMethod::Patch,
        _ => return None,
    };
    catalog::definitions().into_iter().find_map(|definition| {
        if definition.method != wanted {
            return None;
        }
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
) -> Result<(Value, Option<String>), ErasedFailure> {
    if !host.advertises(matched.definition.name) {
        return Err(ErasedFailure::new(None, OperationFailure::AccessDenied));
    }
    identity::reject(&data, &matched.path, &matched.definition)?;
    let identity = host.bound_identity();
    let bound = request::bind(
        &matched.definition,
        &matched.path,
        data,
        &query,
        headers,
        identity.as_ref().map(|(_, scope)| scope.as_str()),
    )?;
    let call = host.bound_call(bound.handler.context, &bound.data)?;
    let mut value = host
        .invoke_backing(matched.definition.name, bound.handler.operation, call)
        .await?;
    response::select(
        &mut value,
        &matched.definition,
        &bound.response,
        &matched.path,
    )?;
    let value = response::success(value, &bound.response);
    let etag = value
        .pointer("/data/edit/etag")
        .or_else(|| value.pointer("/data/etag"))
        .and_then(Value::as_str)
        .map(|etag| format!("\"{etag}\""))
        .or_else(|| {
            value
                .pointer("/data/version")
                .or_else(|| value.pointer("/data/discussion/version"))
                .and_then(Value::as_u64)
                .map(|version| format!("\"{version}\""))
        });
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
