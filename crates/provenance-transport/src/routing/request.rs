use super::invalid;
use axum::http::HeaderMap;
use provenance_core::protocol::failure::ErasedFailure;
use provenance_store::operations::catalog::{
    self, Definition, HandlerBinding, Parameter, QueryRoute, ResponseBinding, SelectorBinding,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub struct BoundRequest {
    pub data: Value,
    pub handler: HandlerBinding,
    pub response: ResponseBinding,
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

pub fn bind(
    definition: &Definition,
    path: &BTreeMap<String, String>,
    mut data: Value,
    query: &BTreeMap<String, String>,
    headers: &HeaderMap,
    scope: Option<&str>,
) -> Result<BoundRequest, ErasedFailure> {
    let selected = selected_query(definition, query)?;
    let parameters = selected.map_or(
        definition.registration.request.parameters.as_slice(),
        |route| route.parameters.as_slice(),
    );
    let parsed_query = parse_query(parameters, query)?;
    let adapter = selected.map_or(definition.registration.request.adapter, |route| {
        route.request.adapter
    });
    if !adapter.object {
        data = Value::Null;
    } else if !data.is_object() {
        return Err(invalid(None));
    }
    if let Some(object) = data.as_object_mut() {
        object.extend(parsed_query);
        bind_path(definition, path, object)?;
        bind_headers(definition, headers, object)?;
        if let Some(field) = definition.registration.request.scope_field {
            object.insert(field.into(), json!(scope.ok_or_else(|| invalid(None))?));
        }
        if let Some(route) = selected {
            if let Some(node_type) = route.request.node_type {
                let field = if route.request.node_types {
                    "node_types"
                } else {
                    "node_type"
                };
                object.insert(
                    field.into(),
                    if route.request.node_types {
                        json!([node_type])
                    } else {
                        json!(node_type)
                    },
                );
            }
        }
    }
    data = (adapter.adapt)(&definition.registration.request, data, path)
        .map_err(|error| invalid(error.field))?;
    Ok(BoundRequest {
        data,
        handler: selected.map_or_else(
            || definition.registration.handler.clone(),
            |route| route.handler.clone(),
        ),
        response: selected.map_or_else(
            || definition.registration.response.clone(),
            |route| route.response.clone(),
        ),
    })
}

fn selected_query<'a>(
    definition: &'a Definition,
    query: &BTreeMap<String, String>,
) -> Result<Option<&'a QueryRoute>, ErasedFailure> {
    let Some(name) = query.get("query") else {
        return Ok(None);
    };
    definition
        .registration
        .queries
        .iter()
        .find(|route| route.name == name)
        .map(Some)
        .ok_or_else(|| invalid(Some("query")))
}

fn parse_query(
    parameters: &[Parameter],
    query: &BTreeMap<String, String>,
) -> Result<Map<String, Value>, ErasedFailure> {
    let mut parsed = Map::new();
    for (name, raw) in query {
        if name == "query" {
            continue;
        }
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.name == name && parameter.location == "query")
            .ok_or_else(|| invalid(Some(name)))?;
        let value =
            catalog::parse_parameter_value(parameter, raw).map_err(|_| invalid(Some(name)))?;
        parsed.insert(name.clone(), value);
    }
    Ok(parsed)
}

fn bind_path(
    definition: &Definition,
    path: &BTreeMap<String, String>,
    object: &mut Map<String, Value>,
) -> Result<(), ErasedFailure> {
    for binding in &definition.registration.request.path {
        let value = path
            .get(binding.parameter)
            .ok_or_else(|| invalid(Some(binding.parameter)))?;
        object.insert(binding.field.into(), json!(value));
    }
    if let Some(binding) = &definition.registration.request.parent {
        let id = path
            .get(binding.id_parameter)
            .ok_or_else(|| invalid(Some(binding.id_parameter)))?;
        object.insert(
            binding.field.into(),
            json!({"node_type":binding.kind,"node_id":id}),
        );
    }
    if let Some(binding) = &definition.registration.request.selector {
        let (parameter, field, selector) = match binding {
            SelectorBinding::Discussion { parameter, field } => {
                (*parameter, *field, "discussion_id")
            }
            SelectorBinding::Legacy { parameter, field } => (*parameter, *field, "thread_id"),
        };
        let id = path
            .get(parameter)
            .ok_or_else(|| invalid(Some(parameter)))?;
        let kind = if selector == "discussion_id" {
            "discussion"
        } else {
            "legacy"
        };
        let mut value = Map::new();
        value.insert("kind".into(), json!(kind));
        value.insert(selector.into(), json!(id));
        object.insert(field.into(), Value::Object(value));
    }
    Ok(())
}

fn bind_headers(
    definition: &Definition,
    headers: &HeaderMap,
    object: &mut Map<String, Value>,
) -> Result<(), ErasedFailure> {
    for binding in &definition.registration.controls.headers {
        let raw = headers
            .get(binding.name)
            .ok_or_else(|| invalid(Some(binding.name)))?
            .to_str()
            .map_err(|_| invalid(Some(binding.name)))?;
        let raw = if binding.trim_quotes {
            raw.trim_matches('"')
        } else {
            raw
        };
        let value = if binding.numeric {
            Value::from(
                raw.parse::<u64>()
                    .map_err(|_| invalid(Some(binding.name)))?,
            )
        } else {
            json!(raw)
        };
        object.insert(binding.field.into(), value);
    }
    Ok(())
}
