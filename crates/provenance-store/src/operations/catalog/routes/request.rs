use crate::operations::catalog::{
    NullClearBinding, RequestAdapter, RequestAdapterError, RequestBinding,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const DIRECT: RequestAdapter = RequestAdapter {
    object: true,
    adapt: direct,
};
const PUBLIC_PATCH: RequestAdapter = RequestAdapter {
    object: true,
    adapt: public_patch,
};
pub(super) const DISCUSSION_START: RequestAdapter = RequestAdapter {
    object: true,
    adapt: discussion_start,
};
pub(super) const DISCUSSION_REPLY: RequestAdapter = RequestAdapter {
    object: true,
    adapt: discussion_reply,
};
pub(super) const DISCUSSION_STATUS: RequestAdapter = RequestAdapter {
    object: true,
    adapt: discussion_status,
};

#[allow(clippy::unnecessary_wraps)]
const fn direct(
    _: &RequestBinding,
    value: Value,
    _: &BTreeMap<String, String>,
) -> Result<Value, RequestAdapterError> {
    Ok(value)
}

pub(super) fn register_public_patch(
    binding: &mut RequestBinding,
    clearable: &[(&'static str, &'static str)],
) {
    binding.null_clears = clearable
        .iter()
        .map(|(field, clear_name)| NullClearBinding { field, clear_name })
        .collect();
    project_public_patch_schema(&mut binding.schema, clearable);
    binding.adapter = PUBLIC_PATCH;
}

fn public_patch(
    binding: &RequestBinding,
    mut value: Value,
    _: &BTreeMap<String, String>,
) -> Result<Value, RequestAdapterError> {
    let object = value
        .as_object_mut()
        .ok_or(RequestAdapterError { field: None })?;
    if object.contains_key("clear_fields") {
        return Err(RequestAdapterError {
            field: Some("clear_fields"),
        });
    }
    let nulls = object
        .iter()
        .filter(|(_, value)| value.is_null())
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let mut clear = Vec::new();
    for name in nulls {
        let Some(field) = binding.null_clears.iter().find(|field| field.field == name) else {
            return Err(RequestAdapterError { field: None });
        };
        object.remove(&name);
        clear.push(json!(field.clear_name));
    }
    if !clear.is_empty() {
        object.insert("clear_fields".into(), Value::Array(clear));
    }
    Ok(value)
}

fn project_public_patch_schema(
    request: &mut Option<Value>,
    clearable: &[(&'static str, &'static str)],
) {
    let Some(schema) = request else {
        return;
    };
    let removed = schema
        .pointer_mut("/properties/data/properties")
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.remove("clear_fields"));
    if let Some(properties) = schema
        .pointer_mut("/properties/data/properties")
        .and_then(Value::as_object_mut)
    {
        for (name, property) in properties {
            if !clearable.iter().any(|(field, _)| field == name) {
                remove_null(property);
            }
        }
    }
    if let Some(required) = schema
        .pointer_mut("/properties/data/required")
        .and_then(Value::as_array_mut)
    {
        required.retain(|name| name != "clear_fields");
    }
    if let Some(removed) = removed {
        remove_orphaned_definitions(schema, &removed);
    }
}

fn remove_null(schema: &mut Value) {
    let Some(object) = schema.as_object_mut() else {
        return;
    };
    if let Some(kind) = object.get_mut("type") {
        if let Some(types) = kind.as_array_mut() {
            types.retain(|value| value != "null");
            if types.len() == 1 {
                *kind = types[0].clone();
            }
        }
    }
    for keyword in ["anyOf", "oneOf"] {
        if let Some(variants) = object.get_mut(keyword).and_then(Value::as_array_mut) {
            variants.retain(|variant| variant.get("type") != Some(&json!("null")));
        }
    }
}

fn remove_orphaned_definitions(schema: &mut Value, removed: &Value) {
    let mut candidates = BTreeSet::new();
    collect_definition_references(removed, &mut candidates);
    let mut retained = BTreeSet::new();
    collect_definition_references(schema, &mut retained);
    let Some(definitions) = schema.get_mut("$defs").and_then(Value::as_object_mut) else {
        return;
    };
    for name in candidates.difference(&retained) {
        definitions.remove(name);
    }
}

fn collect_definition_references(value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if let Some(name) = object
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|reference| reference.strip_prefix("#/$defs/"))
            {
                names.insert(name.to_owned());
            }
            for child in object.values() {
                collect_definition_references(child, names);
            }
        }
        Value::Array(array) => {
            for child in array {
                collect_definition_references(child, names);
            }
        }
        _ => {}
    }
}

fn discussion_start(
    _: &RequestBinding,
    mut value: Value,
    _: &BTreeMap<String, String>,
) -> Result<Value, RequestAdapterError> {
    let object = object(&mut value)?;
    let action = json!({
        "kind":"start", "role":take(object, "role")?, "body":take(object, "body")?
    });
    object.insert("action".into(), action);
    Ok(value)
}

fn discussion_reply(
    _: &RequestBinding,
    mut value: Value,
    path: &BTreeMap<String, String>,
) -> Result<Value, RequestAdapterError> {
    let discussion_id = path.get("discussion_id").ok_or(RequestAdapterError {
        field: Some("discussion_id"),
    })?;
    let object = object(&mut value)?;
    object.remove("discussion_id");
    let action = json!({
        "kind":"reply", "discussion_id":discussion_id,
        "expected_version":take(object, "expected_version")?,
        "role":take(object, "role")?, "body":take(object, "body")?
    });
    object.insert("action".into(), action);
    Ok(value)
}

fn discussion_status(
    _: &RequestBinding,
    mut value: Value,
    path: &BTreeMap<String, String>,
) -> Result<Value, RequestAdapterError> {
    let discussion_id = path.get("discussion_id").ok_or(RequestAdapterError {
        field: Some("discussion_id"),
    })?;
    let object = object(&mut value)?;
    object.remove("discussion_id");
    let action = json!({
        "kind":"set_status", "discussion_id":discussion_id,
        "expected_version":take(object, "expected_version")?,
        "status":take(object, "status")?
    });
    object.insert("action".into(), action);
    Ok(value)
}

fn object(value: &mut Value) -> Result<&mut Map<String, Value>, RequestAdapterError> {
    value
        .as_object_mut()
        .ok_or(RequestAdapterError { field: None })
}

fn take(
    object: &mut Map<String, Value>,
    field: &'static str,
) -> Result<Value, RequestAdapterError> {
    object
        .remove(field)
        .ok_or(RequestAdapterError { field: Some(field) })
}
