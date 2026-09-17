use provenance_core::protocol::{failure::ErasedFailure, read_failure::ReadFailure};
use provenance_store::operations::catalog::{
    Definition, ResponseBinding, ResponseKind, ResponseSelection,
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
        ResponseSelection::PageMember {
            id_parameter,
            id_pointer,
        } => {
            let id = path
                .get(*id_parameter)
                .ok_or_else(|| not_found(definition))?;
            let found = value
                .pointer("/result/entries")
                .and_then(Value::as_array)
                .and_then(|entries| {
                    entries
                        .iter()
                        .find(|entry| entry.pointer(id_pointer).and_then(Value::as_str) == Some(id))
                })
                .cloned()
                .ok_or_else(|| not_found(definition))?;
            value["result"] = found;
            Ok(())
        }
    }
}

fn not_found(definition: &Definition) -> ErasedFailure {
    ErasedFailure::declared(definition.name, ReadFailure::ResourceNotFound, 404)
}

pub fn success(mut value: Value, binding: &ResponseBinding) -> Value {
    let mut meta = Map::new();
    if let Some(object) = value.as_object_mut() {
        move_meta(object, &mut meta);
        object.remove("protocol_version");
        object.remove("operation");
        if binding.kind == ResponseKind::Resource && object.get("found") == Some(&json!(true)) {
            value = object.remove("node").unwrap_or(Value::Null);
        }
    }
    if let Some(result) = value
        .as_object_mut()
        .and_then(|object| object.remove("result"))
    {
        value = result;
        if let Some(object) = value.as_object_mut() {
            move_page_meta(object, &mut meta);
        }
    }
    let data = if binding.kind == ResponseKind::Items {
        if value.is_array() {
            json!({"items":value})
        } else {
            let items = binding
                .items_field
                .and_then(|field| value.as_object_mut()?.remove(field))
                .unwrap_or(value);
            json!({"items":items})
        }
    } else {
        value
    };
    json!({"data":data,"meta":meta})
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
