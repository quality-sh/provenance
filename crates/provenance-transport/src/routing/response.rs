use provenance_core::protocol::{failure::ErasedFailure, read_failure::ReadFailure};
use provenance_store::operations::catalog::{
    Definition, ResponseAdapter, ResponseBinding, ResponseSelection,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub fn select(
    value: &mut Value,
    definition: &Definition,
    binding: &ResponseBinding,
    path: &BTreeMap<String, String>,
) -> Result<(), ErasedFailure> {
    match &binding.selection {
        ResponseSelection::Direct => Ok(()),
        ResponseSelection::ArrayItems {
            owner_parameter,
            owner_field,
        } => {
            let owner = path
                .get(*owner_parameter)
                .ok_or_else(|| not_found(definition))?;
            if let Some(entries) = value.as_array_mut() {
                entries
                    .retain(|entry| entry.get(*owner_field).and_then(Value::as_str) == Some(owner));
            }
            Ok(())
        }
        ResponseSelection::ArrayMember {
            id_parameter,
            owner_parameter,
        } => {
            let id = path
                .get(*id_parameter)
                .ok_or_else(|| not_found(definition))?;
            let owner = owner_parameter
                .as_ref()
                .and_then(|(parameter, field)| path.get(*parameter).map(|value| (*field, value)));
            let found = value
                .as_array()
                .and_then(|entries| {
                    entries.iter().find(|entry| {
                        entry.get("id").and_then(Value::as_str) == Some(id)
                            && owner.is_none_or(|(field, owner)| {
                                entry.get(field).and_then(Value::as_str) == Some(owner)
                            })
                    })
                })
                .cloned()
                .ok_or_else(|| not_found(definition))?;
            *value = found;
            Ok(())
        }
    }
}

fn not_found(definition: &Definition) -> ErasedFailure {
    ErasedFailure::declared(definition.name, ReadFailure::ResourceNotFound, 404)
}

pub fn success(
    mut value: Value,
    definition: &Definition,
    binding: &ResponseBinding,
) -> Result<Value, ErasedFailure> {
    let mut meta = Map::new();
    if let Some(object) = value.as_object_mut() {
        move_meta(object, &mut meta);
        object.remove("protocol_version");
        object.remove("operation");
    }
    let data = match binding.adapter {
        ResponseAdapter::Direct => value,
        ResponseAdapter::Result => take_result(value, definition, &mut meta)?,
        ResponseAdapter::ArrayItems => {
            if !value.is_array() {
                return Err(malformed(definition));
            }
            json!({"items":value})
        }
        ResponseAdapter::ObjectItems(field) => {
            let items = value
                .as_object_mut()
                .and_then(|object| object.remove(field))
                .ok_or_else(|| malformed(definition))?;
            if !items.is_array() {
                return Err(malformed(definition));
            }
            json!({"items":items})
        }
        ResponseAdapter::ResultItems(field) => {
            let mut result = take_result(value, definition, &mut meta)?;
            let items = result
                .as_object_mut()
                .and_then(|object| object.remove(field))
                .ok_or_else(|| malformed(definition))?;
            if !items.is_array() {
                return Err(malformed(definition));
            }
            json!({"items":items})
        }
    };
    Ok(json!({"data":data,"meta":meta}))
}

fn take_result(
    mut value: Value,
    definition: &Definition,
    meta: &mut Map<String, Value>,
) -> Result<Value, ErasedFailure> {
    let result = value
        .as_object_mut()
        .and_then(|object| take_field(object, "result"))
        .ok_or_else(|| malformed(definition))?;
    let mut result = result;
    if let Some(object) = result.as_object_mut() {
        move_page_meta(object, meta);
    }
    Ok(result)
}

fn take_field(object: &mut Map<String, Value>, field: &str) -> Option<Value> {
    object.remove(field)
}

fn malformed(definition: &Definition) -> ErasedFailure {
    ErasedFailure::new(
        Some(definition.name),
        provenance_core::protocol::failure::OperationFailure::Internal,
    )
}

fn move_meta(object: &mut Map<String, Value>, meta: &mut Map<String, Value>) {
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
}

fn move_page_meta(object: &mut Map<String, Value>, meta: &mut Map<String, Value>) {
    for field in ["limit", "has_more", "next_cursor"] {
        if let Some(value) = object.remove(field) {
            meta.insert(field.into(), value);
        }
    }
}
