use super::{schema, ResponseKind};
use provenance_core::protocol::ResponseMeta;
use schemars::generate::Contract;
use serde_json::{json, Value};

pub fn response_envelope(mut payload: Value, kind: ResponseKind) -> Value {
    if kind != ResponseKind::Items {
        remove_response_metadata(
            &mut payload,
            &[
                "protocol_version",
                "operation",
                "stamp",
                "freshness_error",
                "freshness_cause",
                "limit",
                "has_more",
                "next_cursor",
            ],
        );
    }
    let defs = payload.as_object_mut().and_then(|o| o.remove("$defs"));
    payload.as_object_mut().map(|o| o.remove("$schema"));
    let mut meta = schema::<ResponseMeta>(Contract::Serialize);
    namespace_defs(&mut meta, "ResponseMeta");
    let meta_defs = meta.as_object_mut().and_then(|o| o.remove("$defs"));
    meta.as_object_mut().map(|o| o.remove("$schema"));
    let data = match kind {
        ResponseKind::Items => {
            json!({"type":"object","additionalProperties":false,"required":["items"],"properties":{"items":payload}})
        }
        ResponseKind::Resource | ResponseKind::Result => payload,
    };
    let mut result = json!({"type":"object","additionalProperties":false,"required":["data","meta"],"properties":{"data":data,"meta":meta}});
    let mut merged = serde_json::Map::new();
    if let Some(Value::Object(defs)) = defs {
        merged.extend(defs);
    }
    if let Some(Value::Object(defs)) = meta_defs {
        for (name, value) in defs {
            merged.entry(name).or_insert(value);
        }
    }
    if !merged.is_empty() {
        result["$defs"] = Value::Object(merged);
    }
    result
}

fn namespace_defs(value: &mut Value, prefix: &str) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                if let Some(name) = reference.strip_prefix("#/$defs/") {
                    *reference = format!("#/$defs/{prefix}{name}");
                }
            }
            if let Some(Value::Object(defs)) = object.remove("$defs") {
                object.insert(
                    "$defs".into(),
                    Value::Object(
                        defs.into_iter()
                            .map(|(name, schema)| (format!("{prefix}{name}"), schema))
                            .collect(),
                    ),
                );
            }
            for child in object.values_mut() {
                namespace_defs(child, prefix);
            }
        }
        Value::Array(array) => {
            for child in array {
                namespace_defs(child, prefix);
            }
        }
        _ => {}
    }
}

pub fn request_envelope(mut request: Value) -> Value {
    let defs = request.as_object_mut().and_then(|o| o.remove("$defs"));
    request.as_object_mut().map(|o| o.remove("$schema"));
    let mut result = json!({"type":"object","additionalProperties":false,"required":["data"],"properties":{"data":request}});
    if let Some(defs) = defs {
        result["$defs"] = defs;
    }
    result
}

/// Removes a bound field from the properties and the required list of the request data. A
/// body without the field cannot meet a condition on it. When a keyword that stays in the
/// data names the field, the construction of the catalog stops.
pub fn hide_bound_request_field(operation: &str, request: &mut Option<Value>, field: &str) {
    let Some(data) = request
        .as_mut()
        .and_then(|schema| schema.pointer_mut("/properties/data"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    if let Some(properties) = data.get_mut("properties").and_then(Value::as_object_mut) {
        properties.remove(field);
    }
    if let Some(required) = data.get_mut("required").and_then(Value::as_array_mut) {
        required.retain(|name| name.as_str() != Some(field));
    }
    let condition = data.iter().find(|(keyword, value)| {
        keyword.as_str() != "properties" && member_names_field(keyword, value, field)
    });
    if let Some((keyword, value)) = condition {
        panic!(
            "operation `{operation}` hides the field `{field}` from its request body, and the \
             keyword `{keyword}` of that body names the field, or has a reference or a pattern \
             for property names that can name it. A body cannot meet a condition on a field \
             that it does not have: {value}"
        );
    }
}

/// The keywords that can name a field that their text does not show: a reference, and a
/// pattern for property names.
const NAMES_NOT_SHOWN: [&str; 4] = ["$ref", "$dynamicRef", "patternProperties", "propertyNames"];

/// True when the member `name` of a schema, or its value, names `field`.
fn member_names_field(name: &str, value: &Value, field: &str) -> bool {
    name == field || NAMES_NOT_SHOWN.contains(&name) || value_names_field(value, field)
}

/// True when `value` has the name of `field` as a member name or as a string, at each depth.
fn value_names_field(value: &Value, field: &str) -> bool {
    match value {
        Value::String(text) => text == field,
        Value::Array(members) => members
            .iter()
            .any(|member| value_names_field(member, field)),
        Value::Object(members) => members
            .iter()
            .any(|(name, member)| member_names_field(name, member, field)),
        _ => false,
    }
}

fn remove_response_metadata(value: &mut Value, fields: &[&str]) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if let Some(Value::Object(properties)) = object.get_mut("properties") {
        for field in fields {
            properties.remove(*field);
        }
    }
    if let Some(Value::Array(required)) = object.get_mut("required") {
        required.retain(|field| !fields.iter().any(|name| field == *name));
    }
}
