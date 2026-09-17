use super::{identity, invalid};
use axum::http::HeaderMap;
use provenance_core::protocol::failure::ErasedFailure;
use provenance_store::operations::catalog::{Definition, ResponseKind};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

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

pub fn validate_query(
    definition: &Definition,
    query: &BTreeMap<String, String>,
) -> Result<(), ErasedFailure> {
    for name in query.keys() {
        if !definition
            .parameters
            .iter()
            .any(|parameter| parameter.location == "query" && parameter.name == name)
        {
            return Err(invalid(Some(name)));
        }
    }
    if query.contains_key("query") {
        return Ok(());
    }
    if definition.backing.starts_with("list-") && definition.response_kind == ResponseKind::Items {
        for name in query.keys() {
            if !matches!(name.as_str(), "limit" | "cursor") {
                return Err(invalid(Some(name)));
            }
        }
    } else if definition.backing.starts_with("list-") && !query.is_empty() {
        return Err(invalid(query.keys().next().map(String::as_str)));
    }
    Ok(())
}

pub fn get_request(
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
        if name != "query" {
            value.insert(name.clone(), scalar(raw));
        }
    }
    Ok(Value::Object(value))
}

pub fn query_request(
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

pub fn inject_path(
    data: &mut Value,
    path: &BTreeMap<String, String>,
    definition: &Definition,
) -> Result<(), ErasedFailure> {
    let Some(object) = data.as_object_mut() else {
        return Ok(());
    };
    let backing = definition.backing;
    if is_discussion(backing) {
        object.insert("parent".into(), parent(definition, path)?);
    }
    if backing == "review-discussion-messages-v2" {
        let selector = if let Some(id) = path.get("discussion_id") {
            json!({"kind":"discussion","discussion_id":id})
        } else if let Some(id) = path.get("container_id") {
            json!({"kind":"legacy","thread_id":id})
        } else {
            return Err(invalid(Some("discussion_id")));
        };
        object.insert("selector".into(), selector);
    }
    for (name, value) in path {
        if is_discussion(backing)
            && matches!(
                name.as_str(),
                "id" | "discussion_id" | "container_id" | "message_id"
            )
        {
            continue;
        }
        let field = path_field(name, backing);
        if backing != "get" {
            object.insert(field.to_owned(), json!(value));
        }
    }
    Ok(())
}

fn is_discussion(backing: &str) -> bool {
    matches!(
        backing,
        "review-discussions-v2" | "review-discussion-messages-v2" | "write-discussion-v2"
    )
}

fn parent(
    definition: &Definition,
    path: &BTreeMap<String, String>,
) -> Result<Value, ErasedFailure> {
    let id = path.get("id").ok_or_else(|| invalid(Some("id")))?;
    let kind = match definition.path.split('/').nth(1).unwrap_or_default() {
        "sources" => "source",
        "requirements" => "requirement",
        "resolutions" => "resolution",
        "rules" => "rule",
        "topics" => "topic",
        "questions" => "question",
        _ => return Err(invalid(Some("collection"))),
    };
    Ok(json!({"node_type":kind,"node_id":id}))
}

fn path_field<'a>(name: &'a str, backing: &str) -> &'a str {
    if name == "run_id" {
        "run"
    } else if name == "id" && matches!(backing, "create-assertion" | "create-disposition") {
        "proposal_id"
    } else if name == "id" && backing == "evidence" {
        "rule"
    } else if name == "id"
        && matches!(
            backing,
            "submit-requirement-review-v2"
                | "decide-requirement-review-v2"
                | "withdraw-requirement-review-v2"
                | "review-history-v2"
                | "review-history-entry-v2"
                | "review-evidence-v2"
        )
    {
        "requirement_id"
    } else {
        name
    }
}

pub fn inject_headers(
    data: &mut Value,
    definition: &Definition,
    headers: &HeaderMap,
) -> Result<(), ErasedFailure> {
    let Some(object) = data.as_object_mut() else {
        return Ok(());
    };
    if let Some(value) = headers.get("Idempotency-Key") {
        object.insert(
            "request_id".into(),
            json!(header(value, "Idempotency-Key")?),
        );
    }
    if definition.backing == "update-requirement-v2" {
        if let Some(value) = headers.get("If-Match") {
            object.insert(
                "expected_etag".into(),
                json!(header(value, "If-Match")?.trim_matches('"')),
            );
        }
    }
    Ok(())
}

fn header<'a>(
    value: &'a axum::http::HeaderValue,
    name: &'static str,
) -> Result<&'a str, ErasedFailure> {
    value.to_str().map_err(|_| invalid(Some(name)))
}

pub fn shape_discussion_action(
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
        changed_discussion_action(object, definition, path, headers)?
    };
    object.insert("action".into(), action);
    Ok(())
}

fn changed_discussion_action(
    object: &mut Map<String, Value>,
    definition: &Definition,
    path: &BTreeMap<String, String>,
    headers: &HeaderMap,
) -> Result<Value, ErasedFailure> {
    let discussion_id = path
        .get("discussion_id")
        .ok_or_else(|| invalid(Some("discussion_id")))?;
    let version = headers
        .get("If-Match")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim_matches('"').parse::<u64>().ok())
        .ok_or_else(|| invalid(Some("If-Match")))?;
    if definition.name.ends_with("create-discussion-message") {
        Ok(
            json!({"kind":"reply","discussion_id":discussion_id,"expected_version":version,
            "role":take(object, "role")?,"body":take(object, "body")?}),
        )
    } else {
        Ok(
            json!({"kind":"set_status","discussion_id":discussion_id,"expected_version":version,
            "status":take(object, "status")?}),
        )
    }
}

fn take(object: &mut Map<String, Value>, field: &'static str) -> Result<Value, ErasedFailure> {
    object.remove(field).ok_or_else(|| invalid(Some(field)))
}

pub fn require_headers(definition: &Definition, headers: &HeaderMap) -> Result<(), ErasedFailure> {
    for parameter in definition
        .parameters
        .iter()
        .filter(|parameter| parameter.location == "header" && parameter.required)
    {
        if headers.get(parameter.name).is_none() {
            return Err(invalid(Some(parameter.name)));
        }
    }
    Ok(())
}
