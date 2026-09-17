use crate::operations::catalog::{RequestAdapter, RequestAdapterError, RequestBinding};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub(crate) const DIRECT: RequestAdapter = RequestAdapter {
    object: true,
    adapt: direct,
};
pub(super) const NULL: RequestAdapter = RequestAdapter {
    object: false,
    adapt: null,
};
pub(super) const NULLABLE_PATCH: RequestAdapter = RequestAdapter {
    object: true,
    adapt: nullable_patch,
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

fn direct(
    _: &RequestBinding,
    value: Value,
    _: &BTreeMap<String, String>,
) -> Result<Value, RequestAdapterError> {
    Ok(value)
}

fn null(
    _: &RequestBinding,
    _: Value,
    _: &BTreeMap<String, String>,
) -> Result<Value, RequestAdapterError> {
    Ok(Value::Null)
}

fn nullable_patch(
    binding: &RequestBinding,
    mut value: Value,
    _: &BTreeMap<String, String>,
) -> Result<Value, RequestAdapterError> {
    let object = value
        .as_object_mut()
        .ok_or(RequestAdapterError { field: None })?;
    let mut clear = object
        .remove("clear_fields")
        .map(|value| {
            value.as_array().cloned().ok_or(RequestAdapterError {
                field: Some("clear_fields"),
            })
        })
        .transpose()?
        .unwrap_or_default();
    for field in &binding.null_clears {
        if object.get(field.field).is_some_and(Value::is_null) {
            object.remove(field.field);
            let name = json!(field.clear_name);
            if !clear.contains(&name) {
                clear.push(name);
            }
        }
    }
    object.insert("clear_fields".into(), Value::Array(clear));
    Ok(value)
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
