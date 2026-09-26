use serde_json::Value;

/// Whether one body wire field is required by the request schema.
pub(super) fn required(request: &Value, wire_field: &str) -> bool {
    request
        .pointer("/properties/data/required")
        .and_then(Value::as_array)
        .is_some_and(|names| names.iter().any(|name| name.as_str() == Some(wire_field)))
}

/// Whether one schema names only objects, following references and unions.
pub(super) fn object_only(root: &Value, schema: &Value) -> bool {
    let schema = resolve(root, schema);
    if let Some(list) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        let mut any = false;
        for variant in list {
            if is_null(root, variant) {
                continue;
            }
            if !object_only(root, variant) {
                return false;
            }
            any = true;
        }
        return any;
    }
    has_type(schema, "object")
}

pub(super) fn type_label(root: &Value, schema: &Value) -> String {
    let schema = resolve(root, schema);
    if let Some(values) = values(schema) {
        return values.join("|");
    }
    if let Some(kind) = schema.get("type").and_then(Value::as_str) {
        return match kind {
            "array" => schema.get("items").map_or_else(
                || "array".into(),
                |items| format!("array<{}>", type_label(root, items)),
            ),
            "object" => "json".into(),
            other => other.into(),
        };
    }
    if let Some(types) = schema.get("type").and_then(Value::as_array) {
        let labels = types
            .iter()
            .filter_map(Value::as_str)
            .filter(|kind| *kind != "null")
            .collect::<Vec<_>>();
        if !labels.is_empty() {
            return labels.join("|");
        }
    }
    let variants = variants(schema)
        .filter(|variant| !is_null(root, variant))
        .map(|variant| type_label(root, variant))
        .collect::<Vec<_>>();
    if variants.is_empty() {
        "json".into()
    } else {
        let mut labels = variants;
        labels.sort();
        labels.dedup();
        labels.join("|")
    }
}

pub(super) fn array_item_label(root: &Value, schema: &Value) -> Option<String> {
    let schema = effective(root, schema);
    has_type(schema, "array")
        .then(|| schema.get("items"))
        .flatten()
        .map(|items| type_label(root, items))
}

pub(super) fn default(schema: &Value) -> Option<&Value> {
    schema.get("default").filter(|value| !value.is_null())
}

pub(super) fn constraints(schema: &Value) -> Option<String> {
    let minimum = schema.get("minimum").and_then(number);
    let maximum = schema.get("maximum").and_then(number);
    match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => Some(format!("range: {minimum}..{maximum}")),
        (Some(minimum), None) => Some(format!("minimum: {minimum}")),
        (None, Some(maximum)) => Some(format!("maximum: {maximum}")),
        (None, None) => None,
    }
}

fn effective<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
    let schema = resolve(root, schema);
    let mut non_null = variants(schema).filter(|variant| !is_null(root, variant));
    let first = non_null.next();
    if non_null.next().is_none() {
        first.map_or(schema, |variant| resolve(root, variant))
    } else {
        schema
    }
}

fn resolve<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
    schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix("#/$defs/"))
        .and_then(|name| root.get("$defs").and_then(|defs| defs.get(name)))
        .map_or(schema, |resolved| resolve(root, resolved))
}

fn variants(schema: &Value) -> impl Iterator<Item = &Value> {
    schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

fn is_null(root: &Value, schema: &Value) -> bool {
    let schema = resolve(root, schema);
    has_type(schema, "null")
}

fn has_type(schema: &Value, expected: &str) -> bool {
    schema.get("type").is_some_and(|kind| match kind {
        Value::String(kind) => kind == expected,
        Value::Array(kinds) => kinds.iter().any(|kind| kind.as_str() == Some(expected)),
        _ => false,
    })
}

fn values(schema: &Value) -> Option<Vec<String>> {
    let values = schema
        .get("enum")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .or_else(|| schema.get("const").map(std::slice::from_ref))?;
    Some(values.iter().map(value_label).collect())
}

fn value_label(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

fn number(value: &Value) -> Option<String> {
    value
        .as_i64()
        .map(|value| value.to_string())
        .or_else(|| value.as_u64().map(|value| value.to_string()))
        .or_else(|| value.as_f64().map(|value| value.to_string()))
}
