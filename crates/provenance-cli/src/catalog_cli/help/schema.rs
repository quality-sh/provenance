use serde_json::Value;

pub(super) struct BodyField<'a> {
    pub name: &'a str,
    pub schema: &'a Value,
    pub required: bool,
}

pub(super) fn body_fields(request: &Value) -> Vec<BodyField<'_>> {
    let Some(data) = request.pointer("/properties/data") else {
        return Vec::new();
    };
    let required = data
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let mut fields = data
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|properties| properties.iter())
        .map(|(name, schema)| BodyField {
            name,
            schema,
            required: required.contains(&name.as_str()),
        })
        .collect::<Vec<_>>();
    fields.sort_unstable_by_key(|field| field.name);
    fields
}

pub(super) fn type_label(root: &Value, schema: &Value) -> String {
    let schema = resolve(root, schema);
    if let Some(values) = values(schema) {
        return values.join("|");
    }
    if let Some(kind) = schema.get("type").and_then(Value::as_str) {
        return match kind {
            "array" => schema
                .get("items")
                .map(|items| format!("array<{}>", type_label(root, items)))
                .unwrap_or_else(|| "array".into()),
            "object" => "json".into(),
            other => other.into(),
        };
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
    (schema.get("type").and_then(Value::as_str) == Some("array"))
        .then(|| schema.get("items"))
        .flatten()
        .map(|items| type_label(root, items))
}

pub(super) fn accepts_plain(root: &Value, schema: &Value) -> bool {
    let schema = effective(root, schema);
    matches!(
        schema.get("type").and_then(Value::as_str),
        Some("string" | "boolean" | "integer" | "number")
    )
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
    schema.get("type").and_then(Value::as_str) == Some("null")
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
