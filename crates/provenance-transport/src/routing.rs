use crate::StatementHost;
use axum::http::{HeaderMap, Method};
use provenance_core::protocol::failure::{ErasedFailure, InvalidInputReason, OperationFailure};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_store::operations::catalog::{
    self, ContextKind, Definition, HttpMethod, ResponseKind,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

mod identity;

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

pub fn query(raw: Option<&str>) -> Result<BTreeMap<String, String>, ErasedFailure> {
    url::form_urlencoded::parse(raw.unwrap_or_default().as_bytes()).try_fold(
        BTreeMap::new(),
        |mut fields, (name, value)| {
            if fields
                .insert(name.into_owned(), value.into_owned())
                .is_some()
            {
                return Err(invalid(None));
            }
            Ok(fields)
        },
    )
}

pub fn decode_body(bytes: &[u8], expects_body: bool) -> Result<Value, ErasedFailure> {
    if !expects_body {
        return if bytes.is_empty() {
            Ok(json!({}))
        } else {
            Err(invalid(None))
        };
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| invalid(None))?;
    let object = value.as_object().ok_or_else(|| invalid(None))?;
    if object.len() != 1 || !object.contains_key("data") {
        return Err(invalid(None));
    }
    Ok(object["data"].clone())
}

pub async fn invoke(
    host: &StatementHost,
    matched: &Matched,
    mut data: Value,
    query: BTreeMap<String, String>,
    headers: &HeaderMap,
) -> Result<(Value, Option<String>), ErasedFailure> {
    if !host.advertises(&matched.definition.name) {
        return Err(ErasedFailure::new(None, OperationFailure::AccessDenied));
    }
    identity::reject(&data, &matched.path)?;
    require_headers(&matched.definition, headers)?;
    shape_discussion_action(&mut data, &matched.definition, &matched.path, headers)?;
    inject_headers(&mut data, headers)?;
    let mut backing = matched.definition.backing;
    let query_name = query.get("query").map(String::as_str);
    if let Some(name @ ("search" | "stale" | "resolve-symbol" | "trace" | "neighbors" | "impact")) =
        query_name
    {
        backing = name;
        data = query_request(name, &matched.definition, &matched.path, &query)?;
    } else if matches!(matched.definition.method, HttpMethod::Get) {
        data = get_request(&matched.definition, &matched.path, &query)?;
    }
    inject_path(&mut data, &matched.path, &matched.definition)?;
    if matched.definition.inject_scope {
        let scope = host
            .bound_identity()
            .map(|(_, scope)| scope)
            .ok_or_else(|| invalid(None))?;
        data.as_object_mut()
            .ok_or_else(|| invalid(None))?
            .insert("scope_id".into(), json!(scope));
    }
    let call = host.bound_call(matched.definition.context, data)?;
    let mut value = host
        .invoke_backing(&matched.definition.name, backing, call)
        .await?;
    select_addressed(&mut value, &matched.definition, &matched.path)?;
    let mut value = success(value, matched.definition.response_kind, query_name);
    select_page_member(&mut value, &matched.path)?;
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

fn select_addressed(
    value: &mut Value,
    definition: &Definition,
    path: &BTreeMap<String, String>,
) -> Result<(), ErasedFailure> {
    let Some(entries) = value.as_array_mut() else {
        return Ok(());
    };
    if definition.response_kind == ResponseKind::Items {
        if let Some(parent) = path.get("id") {
            entries.retain(|entry| {
                entry
                    .get("proposal_id")
                    .and_then(Value::as_str)
                    .is_none_or(|id| id == parent)
            });
        }
        if let Some(container) = path.get("container_id") {
            entries
                .retain(|entry| entry.get("thread_id").and_then(Value::as_str) == Some(container));
        }
        return Ok(());
    }
    let wanted = path
        .get("fact_id")
        .or_else(|| path.get("message_id"))
        .or_else(|| path.get("id"));
    let Some(wanted) = wanted else {
        return Ok(());
    };
    let found = entries
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(wanted))
        .cloned()
        .ok_or_else(|| {
            ErasedFailure::declared(definition.name, ReadFailure::ResourceNotFound, 404)
        })?;
    *value = found;
    Ok(())
}

fn get_request(
    definition: &Definition,
    path: &BTreeMap<String, String>,
    query: &BTreeMap<String, String>,
) -> Result<Value, ErasedFailure> {
    if definition.backing.starts_with("list-") {
        return Ok(Value::Null);
    }
    if definition.backing == "get" {
        return Ok(json!({"node_type": identity::node_type(definition.path)?, "id":path["id"]}));
    }
    let mut value = Map::new();
    for (name, raw) in query {
        if name == "query" {
            continue;
        }
        value.insert(name.clone(), scalar(raw));
    }
    Ok(Value::Object(value))
}

fn query_request(
    name: &str,
    definition: &Definition,
    path: &BTreeMap<String, String>,
    query: &BTreeMap<String, String>,
) -> Result<Value, ErasedFailure> {
    let mut value = Map::new();
    for (field, raw) in query {
        if field != "query" {
            let parsed = if field == "relations" {
                Value::Array(raw.split(',').map(|part| json!(part)).collect())
            } else {
                scalar(raw)
            };
            value.insert(field.clone(), parsed);
        }
    }
    if let Some(id) = path.get("id") {
        value.insert("id".into(), json!(id));
    }
    if name == "search" {
        value.insert(
            "node_types".into(),
            json!([identity::node_type(definition.path)?]),
        );
    }
    if matches!(name, "trace" | "neighbors" | "impact") {
        value.insert(
            "node_type".into(),
            json!(identity::node_type(definition.path)?),
        );
    }
    Ok(Value::Object(value))
}

fn scalar(raw: &str) -> Value {
    match raw {
        "true" => json!(true),
        "false" => json!(false),
        _ => raw.parse::<u64>().map_or_else(|_| json!(raw), Value::from),
    }
}

fn inject_path(
    data: &mut Value,
    path: &BTreeMap<String, String>,
    definition: &Definition,
) -> Result<(), ErasedFailure> {
    let Some(object) = data.as_object_mut() else {
        return Ok(());
    };
    let backing = definition.backing;
    if matches!(
        backing,
        "review-discussions-v2" | "review-discussion-messages-v2" | "write-discussion-v2"
    ) {
        let id = path.get("id").ok_or_else(|| invalid(Some("id")))?;
        let kind = definition.path.split('/').nth(1).unwrap_or_default();
        let kind = match kind {
            "sources" => "source",
            "requirements" => "requirement",
            "resolutions" => "resolution",
            "rules" => "rule",
            "topics" => "topic",
            "questions" => "question",
            _ => return Err(invalid(Some("collection"))),
        };
        object.insert("parent".into(), json!({"node_type":kind,"node_id":id}));
    }
    for (name, value) in path {
        let field = if name == "run_id" {
            "run"
        } else if name == "id" && matches!(backing, "create-assertion" | "create-disposition") {
            "proposal_id"
        } else if name == "id" && backing == "evidence" {
            "rule"
        } else if name == "id" && backing == "submit-requirement-review-v2" {
            "requirement_id"
        } else if name == "id"
            && matches!(
                backing,
                "review-history-v2" | "review-history-entry-v2" | "review-evidence-v2"
            )
        {
            "requirement_id"
        } else {
            name.as_str()
        };
        if name == "id"
            && matches!(
                backing,
                "review-discussions-v2" | "review-discussion-messages-v2" | "write-discussion-v2"
            )
        {
            continue;
        }
        if name == "id"
            && matches!(
                backing,
                "decide-requirement-review-v2" | "withdraw-requirement-review-v2"
            )
        {
            continue;
        }
        if backing != "get" {
            object.insert(field.to_owned(), json!(value));
        }
    }
    Ok(())
}

fn select_page_member(
    envelope: &mut Value,
    path: &BTreeMap<String, String>,
) -> Result<(), ErasedFailure> {
    let wanted = path.get("message_id").or_else(|| path.get("discussion_id"));
    let Some(wanted) = wanted else {
        return Ok(());
    };
    let Some(entries) = envelope
        .pointer_mut("/data/entries")
        .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };
    let found = entries
        .iter()
        .find(|entry| {
            entry.get("id").and_then(Value::as_str) == Some(wanted)
                || entry
                    .pointer("/discussion/discussion_id")
                    .and_then(Value::as_str)
                    == Some(wanted)
        })
        .cloned()
        .ok_or_else(|| ErasedFailure::new(None, OperationFailure::UnknownOperation))?;
    envelope["data"] = found;
    Ok(())
}

fn inject_headers(data: &mut Value, headers: &HeaderMap) -> Result<(), ErasedFailure> {
    let Some(object) = data.as_object_mut() else {
        return Ok(());
    };
    for (header, field) in [
        ("Idempotency-Key", "request_id"),
        ("If-Match", "expected_etag"),
    ] {
        if let Some(value) = headers.get(header) {
            let value = value.to_str().map_err(|_| invalid(Some(header)))?;
            object.insert(
                field.into(),
                json!(if header == "If-Match" {
                    value.trim_matches('"')
                } else {
                    value
                }),
            );
        }
    }
    Ok(())
}

fn shape_discussion_action(
    data: &mut Value,
    definition: &Definition,
    path: &BTreeMap<String, String>,
    headers: &HeaderMap,
) -> Result<(), ErasedFailure> {
    if definition.backing != "write-discussion-v2" {
        return Ok(());
    }
    let object = data.as_object_mut().ok_or_else(|| invalid(None))?;
    let action = if definition.name.ends_with("create-discussion") {
        json!({"kind":"start","role":take(object, "role")?,"body":take(object, "body")?})
    } else {
        let discussion_id = path
            .get("discussion_id")
            .ok_or_else(|| invalid(Some("discussion_id")))?;
        let version = headers
            .get("If-Match")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim_matches('"').parse::<u64>().ok())
            .ok_or_else(|| invalid(Some("If-Match")))?;
        if definition.name.ends_with("create-discussion-message") {
            json!({"kind":"reply","discussion_id":discussion_id,"expected_version":version,"role":take(object, "role")?,"body":take(object, "body")?})
        } else {
            json!({"kind":"set_status","discussion_id":discussion_id,"expected_version":version,"status":take(object, "status")?})
        }
    };
    object.insert("action".into(), action);
    Ok(())
}

fn take(object: &mut Map<String, Value>, field: &'static str) -> Result<Value, ErasedFailure> {
    object.remove(field).ok_or_else(|| invalid(Some(field)))
}

fn require_headers(definition: &Definition, headers: &HeaderMap) -> Result<(), ErasedFailure> {
    for parameter in definition
        .parameters
        .iter()
        .filter(|p| p.location == "header" && p.required)
    {
        if headers.get(parameter.name).is_none() {
            return Err(invalid(Some(parameter.name)));
        }
    }
    Ok(())
}

pub fn success(mut value: Value, kind: ResponseKind, query: Option<&str>) -> Value {
    let mut meta = Map::new();
    if let Some(object) = value.as_object_mut() {
        for field in [
            "stamp",
            "freshness_error",
            "freshness_cause",
            "limit",
            "has_more",
            "next_cursor",
        ] {
            if let Some(value) = object.remove(field) {
                meta.insert(field.into(), value);
            }
        }
        object.remove("protocol_version");
        object.remove("operation");
        if kind == ResponseKind::Resource && object.get("found") == Some(&json!(true)) {
            value = object.remove("node").unwrap_or(Value::Null);
        }
    }
    if let Some(result) = value
        .as_object_mut()
        .and_then(|object| object.remove("result"))
    {
        value = result;
        if let Some(object) = value.as_object_mut() {
            for field in ["limit", "has_more", "next_cursor"] {
                if let Some(value) = object.remove(field) {
                    meta.insert(field.into(), value);
                }
            }
        }
    }
    let data = if kind == ResponseKind::Items {
        if value.is_array() {
            json!({"items":value})
        } else {
            let fields = match query {
                Some("search") => &["nodes"][..],
                Some("stale") => &["sites"][..],
                Some("resolve-symbol") => &["rules"][..],
                _ => &["items", "entries"][..],
            };
            let items = value
                .as_object_mut()
                .and_then(|object| fields.iter().find_map(|field| object.remove(*field)))
                .unwrap_or(value);
            json!({"items":items})
        }
    } else {
        value
    };
    json!({"data":data,"meta":meta})
}

fn invalid(field: Option<&str>) -> ErasedFailure {
    ErasedFailure::new(
        None,
        OperationFailure::InvalidInput {
            field: field.map(str::to_owned),
            reason: InvalidInputReason::InvalidValue,
        },
    )
}

pub fn context(
    identity: Option<(String, String)>,
    kind: ContextKind,
    request: Value,
) -> Result<Value, ErasedFailure> {
    match kind {
        ContextKind::DataFree => Ok(json!({"request":request})),
        ContextKind::Repository => identity.map(|(repository, _)| json!({"context":{"repository":repository},"request":request})).ok_or_else(|| invalid(None)),
        ContextKind::Scope => identity.map(|(repository, scope)| json!({"context":{"repository":repository,"scope":scope},"request":request})).ok_or_else(|| invalid(None)),
        ContextKind::Scoped => identity.map(|(repository, scope)| json!({"context":{"repository":repository,"scope":scope},"request":request})).ok_or_else(|| invalid(None)),
    }
}
